//! Streaming download/upload support.

use crate::ptp::{ObjectHandle, ReceiveStream};
use crate::Error;
use bytes::Bytes;
use std::ops::ControlFlow;
use std::sync::Arc;

use super::device::MtpDeviceInner;

/// Progress information for transfers.
#[derive(Debug, Clone)]
pub struct Progress {
    /// Bytes transferred so far.
    pub bytes_transferred: u64,
    /// Total bytes (if known).
    pub total_bytes: Option<u64>,
}

impl Progress {
    /// Progress as a percentage (0.0 to 100.0).
    #[must_use]
    pub fn percent(&self) -> f64 {
        self.fraction() * 100.0
    }

    /// Progress as a fraction (0.0 to 1.0).
    #[must_use]
    pub fn fraction(&self) -> f64 {
        self.total_bytes.map_or(1.0, |total| {
            if total == 0 {
                1.0
            } else {
                self.bytes_transferred as f64 / total as f64
            }
        })
    }
}

/// Default idle timeout for cancel drain operations.
///
/// After sending the cancel control request, this is how long we wait
/// for additional data on each pipe before assuming it's clear. Matches
/// the 300ms timeout used by libmtp, which mirrors Windows behavior.
pub const DEFAULT_CANCEL_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(300);

/// A file download in progress with true USB streaming.
///
/// This struct wraps the low-level `ReceiveStream` and provides convenient
/// methods for tracking progress. Data is streamed directly from USB as
/// chunks arrive, without buffering the entire file in memory.
///
/// # Important
///
/// The MTP session is locked while this download is active. You must either
/// consume the entire download or call [`cancel()`](Self::cancel) before
/// dropping it. Dropping mid-download without cancelling corrupts the USB
/// session.
///
/// # Example
///
/// ```rust,no_run
/// use mtp_rs::mtp::MtpDevice;
/// use mtp_rs::ObjectHandle;
/// use tokio::io::AsyncWriteExt;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let device = MtpDevice::open_first().await?;
/// # let storages = device.storages().await?;
/// # let storage = &storages[0];
/// # let handle = ObjectHandle(1);
/// let mut download = storage.download_stream(handle).await?;
/// println!("Downloading {} bytes...", download.size());
///
/// # let mut file = tokio::fs::File::create("output.bin").await?;
/// while let Some(chunk) = download.next_chunk().await {
///     let bytes = chunk?;
///     file.write_all(&bytes).await?;
///     println!("Progress: {:.1}%", download.progress() * 100.0);
/// }
/// # Ok(())
/// # }
/// ```
#[must_use = "dropping a FileDownload mid-transfer corrupts the USB session; \
               consume it fully or call cancel()"]
pub struct FileDownload {
    size: u64,
    bytes_received: u64,
    stream: ReceiveStream,
}

impl FileDownload {
    /// Create a new FileDownload wrapping a ReceiveStream.
    pub(crate) fn new(size: u64, stream: ReceiveStream) -> Self {
        Self {
            size,
            bytes_received: 0,
            stream,
        }
    }

    /// Total file size in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Bytes received so far.
    #[must_use]
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    /// Progress as a fraction (0.0 to 1.0).
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.size == 0 {
            1.0
        } else {
            self.bytes_received as f64 / self.size as f64
        }
    }

    /// Cancel the in-progress download.
    ///
    /// Uses the USB Still Image Class cancel mechanism to stop the transfer
    /// and drain remaining data, leaving the session clean for the next
    /// operation.
    ///
    /// The `idle_timeout` controls how long to wait during pipe drain before
    /// assuming the pipe is clear. 1–2 seconds is typically sufficient.
    ///
    /// If the download is already complete, this is a no-op.
    pub async fn cancel(&mut self, idle_timeout: std::time::Duration) -> Result<(), Error> {
        self.stream.cancel(idle_timeout).await
    }

    /// Get the next chunk of data from USB.
    ///
    /// Returns `None` when the download is complete.
    pub async fn next_chunk(&mut self) -> Option<Result<Bytes, Error>> {
        match self.stream.next_chunk().await {
            Some(Ok(bytes)) => {
                self.bytes_received += bytes.len() as u64;
                Some(Ok(bytes))
            }
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }

    /// Consume the download and iterate with a progress callback.
    ///
    /// Calls `on_progress` after each chunk. Return `ControlFlow::Break(())`
    /// to cancel the download.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use mtp_rs::mtp::MtpDevice;
    /// use mtp_rs::ObjectHandle;
    /// use std::ops::ControlFlow;
    ///
    /// # async fn example() -> Result<(), mtp_rs::Error> {
    /// # let device = MtpDevice::open_first().await?;
    /// # let storages = device.storages().await?;
    /// # let storage = &storages[0];
    /// # let handle = ObjectHandle(1);
    /// let download = storage.download_stream(handle).await?;
    /// let data = download.collect_with_progress(|progress| {
    ///     println!("{:.1}%", progress.percent());
    ///     ControlFlow::Continue(())
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn collect_with_progress<F>(mut self, mut on_progress: F) -> Result<Vec<u8>, Error>
    where
        F: FnMut(Progress) -> ControlFlow<()>,
    {
        let mut data = Vec::with_capacity(self.size as usize);

        while let Some(result) = self.next_chunk().await {
            let chunk = result?;
            data.extend_from_slice(&chunk);

            let progress = Progress {
                bytes_transferred: self.bytes_received,
                total_bytes: Some(self.size),
            };

            if let ControlFlow::Break(()) = on_progress(progress) {
                self.stream.cancel(DEFAULT_CANCEL_TIMEOUT).await?;
                return Err(Error::Cancelled);
            }
        }

        Ok(data)
    }

    /// Collect all remaining data into a `Vec<u8>`.
    ///
    /// This consumes the download and buffers all data in memory.
    pub async fn collect(self) -> Result<Vec<u8>, Error> {
        self.stream.collect().await
    }
}

/// Default window size for [`Storage::download_windowed`](crate::mtp::Storage::download_windowed): 8 MiB.
///
/// Each window is one bounded `GetPartialObject64` transaction that **releases**
/// the one-per-device PTP session the moment it returns. On a Pixel 9 Pro XL an
/// 8 MiB window completes in roughly 80 ms — small enough that a concurrent
/// folder listing or navigation slips in between windows at its natural cost,
/// yet large enough to keep throughput high. (For contrast: aborting a
/// held-open multi-GB [`download_stream`](crate::mtp::Storage::download_stream)
/// to free the session takes ~35 s, because the USB cancel must drain the whole
/// backlog.)
///
/// This is a documented suggestion, not a baked-in policy: pass your own
/// `window_size` to [`download_windowed`](crate::mtp::Storage::download_windowed)
/// to tune the responsiveness/throughput tradeoff for your device and workload.
pub const DEFAULT_DOWNLOAD_WINDOW: u32 = 8 * 1024 * 1024;

/// A large-file reader that fetches the file as a sequence of bounded windows,
/// **releasing the PTP session between every window**.
///
/// MTP allows exactly one PTP session per device, and a streaming
/// [`FileDownload`] holds that session open for the *entire* file — so while a
/// big [`download_stream`](crate::mtp::Storage::download_stream) is in flight, no
/// other operation (a folder listing, navigation) can touch the device until the
/// read finishes or is cancelled (and cancelling a multi-GB read costs ~35 s to
/// drain). `WindowedDownload` solves that: each
/// [`next_window()`](Self::next_window) is a single, short `GetPartialObject64`
/// transaction that completes and frees the session, so a listing issued between
/// two windows just works.
///
/// # The consumer owns the policy
///
/// `WindowedDownload` owns the *bookkeeping* (total size, current offset, window
/// sizing, EOF detection) — that's the reusable value. It deliberately owns **no
/// policy**: there's no pause, debounce, rate-limit, or priority gate baked in.
/// Whatever a consumer wants to do "while the session is free" — service a
/// pending listing, check a cancel flag, throttle — it does *between*
/// `next_window()` calls. That separation is the whole design: the library keeps
/// the mechanism unopinionated, the consumer interposes its own logic.
///
/// # Stopping early needs no `cancel()`
///
/// Unlike [`FileDownload`] (which holds the session open and **must** be
/// consumed fully or [`cancel()`](FileDownload::cancel)led before drop, or it
/// corrupts the USB session), a `WindowedDownload` holds nothing between windows.
/// To stop early, just stop calling `next_window()` and drop it — there's no
/// in-flight transfer to drain, so [`Drop`] is a no-op. (If a `next_window()`
/// future is itself dropped mid-call, the session self-heals: the abandoned
/// transaction is drained by the next operation via the crate's
/// `TransactionScope` recovery.)
///
/// # Example
///
/// ```rust,no_run
/// use mtp_rs::mtp::{MtpDevice, DEFAULT_DOWNLOAD_WINDOW};
/// use mtp_rs::ObjectHandle;
/// use tokio::io::AsyncWriteExt;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let device = MtpDevice::open_first().await?;
/// # let storages = device.storages().await?;
/// # let storage = &storages[0];
/// # let handle = ObjectHandle(1);
/// let mut download = storage.download_windowed(handle, DEFAULT_DOWNLOAD_WINDOW).await?;
/// println!("Reading {} bytes, {} per window...", download.size(), DEFAULT_DOWNLOAD_WINDOW);
///
/// let mut file = tokio::fs::File::create("output.bin").await?;
/// while let Some(window) = download.next_window().await {
///     let bytes = window?;
///     file.write_all(&bytes).await?;
///
///     // The session is FREE right here — do other device work between windows.
///     // e.g. service a pending folder listing, or check a cancel flag and stop.
/// }
/// # Ok(())
/// # }
/// ```
pub struct WindowedDownload {
    inner: Arc<MtpDeviceInner>,
    handle: ObjectHandle,
    /// Full object size, cached at construction so progress/ETA stays anchored.
    total_size: u64,
    /// Byte offset of the next window to fetch.
    offset: u64,
    /// Max bytes requested per window (a `GetPartialObject64` `max_bytes`, u32).
    window_size: u32,
}

impl WindowedDownload {
    /// Create a windowed download starting at `start_offset`.
    ///
    /// `window_size` is clamped to at least 1 byte: a zero window can't make
    /// progress and would otherwise read as a stall.
    pub(crate) fn new(
        inner: Arc<MtpDeviceInner>,
        handle: ObjectHandle,
        total_size: u64,
        start_offset: u64,
        window_size: u32,
    ) -> Self {
        Self {
            inner,
            handle,
            total_size,
            offset: start_offset,
            window_size: window_size.max(1),
        }
    }

    /// Full object size in bytes.
    ///
    /// Always the size of the **whole** file, even for a download started at a
    /// non-zero offset, so a consumer's progress and ETA stay anchored to the
    /// complete file rather than the remaining tail.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.total_size
    }

    /// Byte offset of the next window to be read (i.e. how many bytes have been
    /// returned so far for a download started at offset 0).
    #[must_use]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Read the next window via one bounded `GetPartialObject64` transaction,
    /// **releasing the PTP session on return**.
    ///
    /// Returns:
    /// - `Some(Ok(bytes))` — the next window. It's clamped to the bytes
    ///   remaining, so the final window is exactly the short tail. A device may
    ///   legally return *fewer* bytes than requested mid-file; that's honored —
    ///   the offset advances by the bytes actually returned, so the next call
    ///   continues from the right place.
    /// - `None` — clean end of file: the offset has reached the full size (this
    ///   is also the first result for an empty file, or for a download started
    ///   at `offset == size`). No USB transaction is issued in that case.
    /// - `Some(Err(_))` — a transfer error, **or** a device stall: a 0-byte read
    ///   while bytes still remain (`offset < size`) is reported as
    ///   [`Error::InvalidData`], not silently treated as EOF and not spun on, so
    ///   a misbehaving device surfaces instead of looping forever.
    pub async fn next_window(&mut self) -> Option<Result<Vec<u8>, Error>> {
        // EOF: nothing left. Covers the empty-file and offset==size cases too,
        // and issues no USB transaction.
        if self.offset >= self.total_size {
            return None;
        }

        // Clamp the request to the bytes that remain (so the final window is the
        // exact short tail) and to the configured window size. Both bounds are
        // <= u32 (window_size is u32), so the GetPartialObject64 max_bytes never
        // overflows.
        let remaining = self.total_size - self.offset;
        let want = u32::try_from(remaining.min(u64::from(self.window_size))).unwrap_or(u32::MAX);

        match self
            .inner
            .session
            .get_partial_object_64(self.handle, self.offset, want)
            .await
        {
            Ok(bytes) => {
                if bytes.is_empty() {
                    // We asked for >0 bytes (offset < total_size) but the device
                    // returned none. That's a device stall, not EOF — surface it
                    // rather than returning None (which a caller would read as a
                    // clean, complete file) or spinning forever on empty windows.
                    return Some(Err(Error::invalid_data(format!(
                        "device returned 0 bytes at offset {} of {} (expected up to {want}); \
                         treating as a stall rather than end-of-file",
                        self.offset, self.total_size
                    ))));
                }
                // A short non-zero read is legal: advance by what actually came
                // back, not by what we asked for.
                self.offset += bytes.len() as u64;
                Some(Ok(bytes))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::ControlFlow;

    #[test]
    fn progress_calculations() {
        let cases = [
            (50, Some(100), 50.0, 0.5),
            (100, Some(100), 100.0, 1.0),
            (25, Some(100), 25.0, 0.25),
            (0, Some(0), 100.0, 1.0), // Empty file
            (50, None, 100.0, 1.0),   // Unknown total defaults to complete
        ];
        for (transferred, total, expected_pct, expected_frac) in cases {
            let p = Progress {
                bytes_transferred: transferred,
                total_bytes: total,
            };
            assert_eq!(
                p.percent(),
                expected_pct,
                "percent failed for {transferred}/{total:?}"
            );
            assert_eq!(
                p.fraction(),
                expected_frac,
                "fraction failed for {transferred}/{total:?}"
            );
        }

        // Large numbers
        let large = Progress {
            bytes_transferred: u64::MAX / 2,
            total_bytes: Some(u64::MAX),
        };
        let frac = large.fraction();
        assert!(frac > 0.49 && frac < 0.51);
    }

    #[tokio::test]
    async fn test_collect_with_progress_cancel_cleans_up() {
        use crate::ptp::{
            pack_u16, pack_u32, ContainerType, ObjectHandle, OperationCode, PtpSession,
            ResponseCode,
        };
        use crate::transport::mock::MockTransport;
        use std::sync::Arc;

        // Helper to build a response container
        fn response(tx_id: u32, code: ResponseCode) -> Vec<u8> {
            let mut buf = Vec::with_capacity(12);
            buf.extend_from_slice(&pack_u32(12));
            buf.extend_from_slice(&pack_u16(ContainerType::Response.to_code()));
            buf.extend_from_slice(&pack_u16(code.into()));
            buf.extend_from_slice(&pack_u32(tx_id));
            buf
        }

        // Helper to build a data container
        fn data(tx_id: u32, code: OperationCode, payload: &[u8]) -> Vec<u8> {
            let len = 12 + payload.len();
            let mut buf = Vec::with_capacity(len);
            buf.extend_from_slice(&pack_u32(len as u32));
            buf.extend_from_slice(&pack_u16(ContainerType::Data.to_code()));
            buf.extend_from_slice(&pack_u16(code.into()));
            buf.extend_from_slice(&pack_u32(tx_id));
            buf.extend_from_slice(payload);
            buf
        }

        let mock = Arc::new(MockTransport::new());
        let transport: Arc<dyn crate::transport::Transport> = Arc::clone(&mock) as _;
        mock.queue_response(response(0, ResponseCode::Ok)); // OpenSession

        let file_data = vec![1u8; 1000];
        let file_size = file_data.len() as u64;
        mock.queue_response(data(1, OperationCode::GetObject, &file_data));

        let session = Arc::new(PtpSession::open(transport, 1).await.unwrap());
        let stream = session.get_object_stream(ObjectHandle(1)).await.unwrap();
        let download = FileDownload::new(file_size, stream);

        // Break after first chunk
        let result = download
            .collect_with_progress(|_progress| ControlFlow::Break(()))
            .await;

        assert!(matches!(result, Err(Error::Cancelled)));

        // Verify cancel_transfer was called with the correct transaction ID
        let cancel_calls = mock.get_cancel_calls();
        assert_eq!(cancel_calls, vec![1]);
    }
}
