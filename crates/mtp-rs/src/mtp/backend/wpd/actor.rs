//! The per-device COM worker thread and its message protocol.
//!
//! WPD COM pointers are apartment-affine and `!Send`, so one dedicated [`std::thread`] owns the
//! [`WpdDevice`] and does *all* COM work; everything else talks to it over channels. The thread
//! `CoInitializeEx`es an MTA, opens the device, then serves one [`Request`] at a time. [`WpdHandle`]
//! (held by the backend) carries only channel ends, so it is `Send + Sync` with no `unsafe`.

use super::com::{self, WpdDevice};
use super::props::map_hresult;
use crate::cancel::CancelToken;
use crate::mtp::backend::ByteRange;
use crate::mtp::object::NewObjectInfo;
use crate::mtp::{
    Capabilities, DeviceInfo, Error, ObjectHandle, ObjectInfo, StorageId, StorageInfo,
};
use bytes::Bytes;
use futures::channel::{mpsc, oneshot};
use futures::executor::block_on;
use futures::SinkExt;
use std::sync::mpsc as std_mpsc;
use std::thread::JoinHandle;

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// Bytes read from the device per `IStream::Read`.
const CHUNK: usize = 256 * 1024;
/// How many chunks the download channel buffers before back-pressuring the worker.
const DATA_BOUND: usize = 4;

/// Which device the worker should open.
pub(crate) enum OpenSpec {
    /// The first enumerated device.
    First,
    /// The device whose serial number matches.
    Serial(String),
}

/// The streaming-download reply: full object size plus the channel the chunks arrive on.
pub(crate) struct DownloadStart {
    pub(crate) size: u64,
    pub(crate) data: mpsc::Receiver<Result<Bytes, Error>>,
}

/// A unit of work for the COM worker. Each carries the oneshot it replies on.
pub(crate) enum Request {
    Storages(oneshot::Sender<Result<Vec<StorageInfo>, Error>>),
    StorageInfo(StorageId, oneshot::Sender<Result<StorageInfo, Error>>),
    List {
        storage: StorageId,
        parent: Option<ObjectHandle>,
        cancel: Option<CancelToken>,
        reply: oneshot::Sender<Result<Vec<ObjectInfo>, Error>>,
    },
    ObjectInfo(ObjectHandle, oneshot::Sender<Result<ObjectInfo, Error>>),
    Download {
        obj: ObjectHandle,
        range: ByteRange,
        reply: oneshot::Sender<Result<DownloadStart, Error>>,
    },
    ReadRange {
        obj: ObjectHandle,
        offset: u64,
        len: Option<u32>,
        reply: oneshot::Sender<Result<Vec<u8>, Error>>,
    },
    CreateFolder {
        storage: StorageId,
        parent: Option<ObjectHandle>,
        name: String,
        reply: oneshot::Sender<Result<ObjectHandle, Error>>,
    },
    Upload {
        storage: StorageId,
        parent: Option<ObjectHandle>,
        info: NewObjectInfo,
        data: Vec<u8>,
        reply: oneshot::Sender<Result<ObjectHandle, Error>>,
    },
    Delete {
        obj: ObjectHandle,
        reply: oneshot::Sender<Result<(), Error>>,
    },
    Rename {
        obj: ObjectHandle,
        name: String,
        reply: oneshot::Sender<Result<(), Error>>,
    },
    MoveObject {
        obj: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: StorageId,
        reply: oneshot::Sender<Result<(), Error>>,
    },
    CopyObject {
        obj: ObjectHandle,
        new_parent: ObjectHandle,
        new_storage: StorageId,
        reply: oneshot::Sender<Result<ObjectHandle, Error>>,
    },
    Shutdown,
}

/// The backend's handle to a running worker. `Send + Sync`, holds only channel ends.
pub(crate) struct WpdHandle {
    req_tx: std_mpsc::Sender<Request>,
    join: Option<JoinHandle<()>>,
}

impl WpdHandle {
    /// Spawn a worker, open the device, and return the handle plus the device's cached identity.
    pub(crate) async fn spawn(spec: OpenSpec) -> Result<(Self, DeviceInfo, Capabilities), Error> {
        let (req_tx, req_rx) = std_mpsc::channel::<Request>();
        let (startup_tx, startup_rx) =
            oneshot::channel::<Result<(DeviceInfo, Capabilities), Error>>();

        let join = std::thread::Builder::new()
            .name("wpd-com-worker".into())
            .spawn(move || worker_main(spec, startup_tx, req_rx))
            .map_err(|e| Error::Io {
                message: format!("failed to spawn WPD worker thread: {e}"),
            })?;

        let (device_info, capabilities) = startup_rx
            .await
            .map_err(|_| Error::Disconnected)? // worker died before replying
            ?; // the open result itself

        Ok((
            Self {
                req_tx,
                join: Some(join),
            },
            device_info,
            capabilities,
        ))
    }

    /// Send a request and await its reply. Maps a dead worker to [`Error::Disconnected`].
    pub(crate) async fn call<T, F>(&self, make: F) -> Result<T, Error>
    where
        F: FnOnce(oneshot::Sender<Result<T, Error>>) -> Request,
    {
        let (tx, rx) = oneshot::channel();
        self.req_tx
            .send(make(tx))
            .map_err(|_| Error::Disconnected)?;
        rx.await.map_err(|_| Error::Disconnected)?
    }

    /// Best-effort shutdown signal (used by `close()`; `Drop` also sends one).
    pub(crate) fn shutdown(&self) {
        let _ = self.req_tx.send(Request::Shutdown);
    }
}

impl Drop for WpdHandle {
    fn drop(&mut self) {
        // Ask the worker to stop and wait for it to release COM + CoUninitialize on its own thread.
        let _ = self.req_tx.send(Request::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// The worker thread entry point: COM init → open → serve requests → COM teardown.
fn worker_main(
    spec: OpenSpec,
    startup: oneshot::Sender<Result<(DeviceInfo, Capabilities), Error>>,
    req_rx: std_mpsc::Receiver<Request>,
) {
    // SAFETY: this is the only thread that touches these COM objects; MTA matches the spike.
    unsafe {
        if let Err(e) = CoInitializeEx(None, COINIT_MULTITHREADED).ok() {
            let _ = startup.send(Err(map_hresult(e)));
            return;
        }
    }

    let mut dev = match unsafe { open_device(spec) } {
        Ok(dev) => {
            let _ = startup.send(Ok((dev.device_info().clone(), *dev.capabilities())));
            dev
        }
        Err(e) => {
            let _ = startup.send(Err(e));
            unsafe { CoUninitialize() };
            return;
        }
    };

    while let Ok(req) = req_rx.recv() {
        match req {
            Request::Shutdown => break,
            Request::Storages(reply) => {
                let _ = reply.send(unsafe { dev.storages() });
            }
            Request::StorageInfo(storage, reply) => {
                let _ = reply.send(unsafe { dev.storage_info(storage) });
            }
            Request::List {
                storage,
                parent,
                cancel,
                reply,
            } => {
                let _ = reply.send(unsafe { dev.list(storage, parent, cancel.as_ref()) });
            }
            Request::ObjectInfo(obj, reply) => {
                let _ = reply.send(unsafe { dev.object_info(obj) });
            }
            Request::Download { obj, range, reply } => handle_download(&mut dev, obj, range, reply),
            Request::ReadRange {
                obj,
                offset,
                len,
                reply,
            } => {
                let _ = reply.send(handle_read_range(&mut dev, obj, offset, len));
            }
            Request::CreateFolder {
                storage,
                parent,
                name,
                reply,
            } => {
                let _ = reply.send(unsafe { dev.create_folder(storage, parent, &name) });
            }
            Request::Upload {
                storage,
                parent,
                info,
                data,
                reply,
            } => {
                let _ = reply.send(unsafe { dev.upload(storage, parent, &info, &data) });
            }
            Request::Delete { obj, reply } => {
                let _ = reply.send(unsafe { dev.delete(obj) });
            }
            Request::Rename { obj, name, reply } => {
                let _ = reply.send(unsafe { dev.rename(obj, &name) });
            }
            Request::MoveObject {
                obj,
                new_parent,
                new_storage,
                reply,
            } => {
                let _ = reply.send(unsafe { dev.move_object(obj, new_parent, new_storage) });
            }
            Request::CopyObject {
                obj,
                new_parent,
                new_storage,
                reply,
            } => {
                let _ = reply.send(unsafe { dev.copy_object(obj, new_parent, new_storage) });
            }
        }
    }

    // Drop the device (releases all COM interfaces) before uninitializing COM on this thread.
    drop(dev);
    unsafe { CoUninitialize() };
}

/// Resolve an [`OpenSpec`] to an open [`WpdDevice`].
unsafe fn open_device(spec: OpenSpec) -> Result<WpdDevice, Error> {
    let entries = com::enumerate()?;
    match spec {
        OpenSpec::First => {
            let first = entries.first().ok_or(Error::NoDevice)?;
            WpdDevice::open(&first.pnp_id)
        }
        OpenSpec::Serial(serial) => {
            for entry in &entries {
                if let Ok(dev) = WpdDevice::open(&entry.pnp_id) {
                    if dev.device_info().serial_number == serial {
                        return Ok(dev);
                    }
                    // else: `dev` drops here, closing the non-matching device.
                }
            }
            Err(Error::NoDevice)
        }
    }
}

/// Stream an object's bytes into a bounded channel, honoring the requested [`ByteRange`].
///
/// The full object size and the receiver are sent back *before* the read loop starts, so the
/// consumer can begin pulling immediately. Dropping/closing the receiver makes the next `send` fail,
/// which stops the loop and releases the `IStream` — that is the WPD cancel (no SIC needed).
fn handle_download(
    dev: &mut WpdDevice,
    obj: ObjectHandle,
    range: ByteRange,
    reply: oneshot::Sender<Result<DownloadStart, Error>>,
) {
    let size = match unsafe { dev.object_size(obj) } {
        Ok(s) => s,
        Err(e) => {
            let _ = reply.send(Err(e));
            return;
        }
    };
    let offset = range.offset();
    if offset > size {
        let _ = reply.send(Err(Error::invalid_data(format!(
            "download offset {offset} is past the object size {size}"
        ))));
        return;
    }

    let stream = match unsafe { dev.open_stream(obj) } {
        Ok(s) => s,
        Err(e) => {
            let _ = reply.send(Err(e));
            return;
        }
    };
    if let Err(e) = unsafe { com::stream_seek(&stream, offset) } {
        let _ = reply.send(Err(e));
        return;
    }

    let (mut tx, rx) = mpsc::channel::<Result<Bytes, Error>>(DATA_BOUND);
    if reply.send(Ok(DownloadStart { size, data: rx })).is_err() {
        return; // consumer already gave up
    }

    // Cap total bytes for a bounded `Range`; `Full`/`From` read to EOF.
    let mut remaining: Option<u64> = match range {
        ByteRange::Range { len, .. } => Some(len),
        _ => None,
    };
    let mut buf = vec![0u8; CHUNK];
    loop {
        let want = match remaining {
            Some(r) => (r as usize).min(buf.len()),
            None => buf.len(),
        };
        if want == 0 {
            break;
        }
        let n = match unsafe { com::stream_read(&stream, &mut buf[..want]) } {
            Ok(0) => break, // EOF
            Ok(n) => n,
            Err(e) => {
                let _ = block_on(tx.send(Err(e)));
                break;
            }
        };
        if block_on(tx.send(Ok(Bytes::copy_from_slice(&buf[..n])))).is_err() {
            break; // consumer dropped → cancel
        }
        if let Some(r) = remaining.as_mut() {
            *r -= n as u64;
        }
    }
    // `tx` drops here → channel closes → consumer sees `None` (clean EOF).
}

/// Buffered single-shot read of `[offset, offset+len)` (or to EOF when `len` is `None`).
fn handle_read_range(
    dev: &mut WpdDevice,
    obj: ObjectHandle,
    offset: u64,
    len: Option<u32>,
) -> Result<Vec<u8>, Error> {
    let stream = unsafe { dev.open_stream(obj) }?;
    unsafe { com::stream_seek(&stream, offset) }?;

    let cap = len.map(|l| l as usize);
    let mut out = Vec::with_capacity(cap.unwrap_or(0));
    let mut buf = vec![0u8; CHUNK];
    loop {
        let want = match cap {
            Some(c) => (c - out.len()).min(buf.len()),
            None => buf.len(),
        };
        if want == 0 {
            break;
        }
        let n = unsafe { com::stream_read(&stream, &mut buf[..want]) }?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}
