//! Backend-conformance suite: behavioral tests written against the **public** backend-neutral
//! `mtp::` API. The same tests are meant to run against every backend; this file runs them against
//! the only backend available without hardware — `UsbBackend` driving the virtual device.
//!
//! These tests are the cross-backend parity contract: list, round-trip (upload → list → download →
//! verify → delete), ranged/resumable download, windowed download, create folder, rename, move,
//! copy, object info, thumbnail, and cancellation (both `cancel()` and drop). They only touch
//! `mtp_rs::` public items — no PTP/USB internals — so a future WPD backend can be pointed at the
//! exact same assertions.
//!
//! Run with: `cargo test -p mtp-rs --features virtual-device --test conformance`.

#![cfg(feature = "virtual-device")]

use bytes::Bytes;
use mtp_rs::transport::virtual_device::config::{VirtualDeviceConfig, VirtualStorageConfig};
use mtp_rs::{ByteRange, Error, MtpDevice, NewObjectInfo, ObjectHandle, Storage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Build a writable single-storage virtual device backed by a fresh temp dir.
///
/// Returns the open device plus the `TempDir` guard (kept alive for the test's duration).
async fn open_device(serial: &str) -> (MtpDevice, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let config = VirtualDeviceConfig {
        manufacturer: "TestCorp".into(),
        model: "Conformance Phone".into(),
        serial: serial.into(),
        storages: vec![VirtualStorageConfig {
            description: "Internal Storage".into(),
            capacity: 1024 * 1024 * 1024,
            backing_dir: dir.path().to_path_buf(),
            read_only: false,
        }],
        supports_rename: true,
        event_poll_interval: Duration::ZERO,
        watch_backing_dirs: false,
    };
    let device = MtpDevice::builder().open_virtual(config).await.unwrap();
    (device, dir)
}

/// Stream a byte slice as an owned upload stream.
fn upload_stream(
    data: &[u8],
) -> impl futures::Stream<Item = Result<Bytes, std::io::Error>> + Unpin {
    futures::stream::once(futures::future::ok(Bytes::copy_from_slice(data)))
}

/// Upload `data` as `name` into `parent`, returning the new handle.
async fn upload(
    storage: &Storage,
    parent: Option<ObjectHandle>,
    name: &str,
    data: &[u8],
) -> ObjectHandle {
    let info = NewObjectInfo::file(name, data.len() as u64);
    storage
        .upload(parent, info, upload_stream(data))
        .await
        .expect("upload should succeed")
}

#[tokio::test]
async fn device_info_and_capabilities_are_neutral() {
    let (device, _dir) = open_device("conf-info").await;

    let info = device.device_info();
    assert_eq!(info.manufacturer, "TestCorp");
    assert_eq!(info.model, "Conformance Phone");
    assert_eq!(info.serial_number, "conf-info");

    // The virtual device advertises the full op set, so these should all be available.
    let caps = device.capabilities();
    assert!(caps.can_upload);
    assert!(caps.can_delete);
    assert!(caps.can_create_folder);
    assert!(caps.can_move);
    assert!(caps.can_copy);
    assert!(caps.supports_partial_download);
    assert!(device.supports_rename());
    assert!(device.supports_upload());
}

#[tokio::test]
async fn storages_carry_neutral_info() {
    let (device, _dir) = open_device("conf-storages").await;
    let storages = device.storages().await.unwrap();
    assert_eq!(storages.len(), 1);
    let info = storages[0].info();
    assert_eq!(info.description, "Internal Storage");
    assert!(info.total_capacity > 0);
    assert!(info.is_writable);
    // The id round-trips: fetching by it yields the same storage.
    assert_eq!(storages[0].id(), info.id);
}

#[tokio::test]
async fn round_trip_upload_list_download_delete() {
    let (device, _dir) = open_device("conf-roundtrip").await;
    let storage = &device.storages().await.unwrap()[0];

    let content = b"the quick brown fox jumps over the lazy dog".repeat(50);
    let handle = upload(storage, None, "round.bin", &content).await;

    // It shows up in a root listing.
    let listed = storage.list_objects(None).await.unwrap();
    let found = listed
        .iter()
        .find(|o| o.filename == "round.bin")
        .expect("uploaded file should appear in the listing");
    assert!(found.is_file());
    assert_eq!(found.size, content.len() as u64);

    // object_info resolves the same metadata.
    let info = storage.get_object_info(handle).await.unwrap();
    assert_eq!(info.filename, "round.bin");
    assert_eq!(info.size, content.len() as u64);

    // Streaming download of the whole file matches byte-for-byte.
    let dl = storage.download(handle, ByteRange::Full).await.unwrap();
    assert_eq!(dl.size(), content.len() as u64);
    let got = dl.collect().await.unwrap();
    assert_eq!(got, content);

    // Buffered convenience matches too.
    let buffered = storage.download_to_vec(handle).await.unwrap();
    assert_eq!(buffered, content);

    // Delete removes it.
    storage.delete(handle).await.unwrap();
    assert!(storage.get_object_info(handle).await.is_err());
}

#[tokio::test]
async fn ranged_and_resumable_download() {
    let (device, _dir) = open_device("conf-ranged").await;
    let storage = &device.storages().await.unwrap()[0];

    let content: Vec<u8> = (0..4096u32).map(|i| i as u8).collect();
    let handle = upload(storage, None, "ranged.bin", &content).await;

    // A bounded range via the buffered primitive.
    let mid = storage.read_range(handle, 1000, 500).await.unwrap();
    assert_eq!(mid, &content[1000..1500]);

    // Resume from an offset via the streaming primitive: size() stays the FULL size.
    let offset = 1024u64;
    let tail = storage
        .download(handle, ByteRange::From(offset))
        .await
        .unwrap();
    assert_eq!(
        tail.size(),
        content.len() as u64,
        "size() reports the whole file even for a resumed download"
    );
    let tail_bytes = tail.collect().await.unwrap();
    assert_eq!(tail_bytes, &content[offset as usize..]);

    // offset == size yields a clean, empty stream.
    let empty = storage
        .download(handle, ByteRange::From(content.len() as u64))
        .await
        .unwrap();
    assert!(empty.collect().await.unwrap().is_empty());

    // offset > size is a precondition error, before any I/O.
    let err = storage
        .download(handle, ByteRange::From(content.len() as u64 + 1))
        .await;
    assert!(matches!(err, Err(Error::InvalidData { .. })));
}

#[tokio::test]
async fn windowed_download_reassembles_whole_file() {
    let (device, _dir) = open_device("conf-windowed").await;
    let storage = &device.storages().await.unwrap()[0];

    let content: Vec<u8> = (0..5000u32).map(|i| (i * 7) as u8).collect();
    let handle = upload(storage, None, "windowed.bin", &content).await;

    let mut dl = storage
        .download_windowed(handle, ByteRange::Full, 512)
        .await
        .unwrap();
    assert_eq!(dl.size(), content.len() as u64);

    let mut assembled = Vec::new();
    while let Some(window) = dl.next_window().await {
        assembled.extend_from_slice(&window.unwrap());
    }
    assert_eq!(assembled, content);

    // A windowed resume from an offset covers exactly the tail.
    let offset = 2048u64;
    let mut resume = storage
        .download_windowed(handle, ByteRange::From(offset), 256)
        .await
        .unwrap();
    let mut tail = Vec::new();
    while let Some(window) = resume.next_window().await {
        tail.extend_from_slice(&window.unwrap());
    }
    assert_eq!(tail, &content[offset as usize..]);
}

#[tokio::test]
async fn create_folder_rename_move_copy() {
    let (device, _dir) = open_device("conf-fsops").await;
    let storage = &device.storages().await.unwrap()[0];

    // Three folders at the root. `copy_dst` and `move_dst` are kept separate so the copy and the
    // move don't collide on the same filename (a same-name move into a folder overwrites, which is
    // device behavior, not what this test is checking).
    let src = storage.create_folder(None, "src").await.unwrap();
    let copy_dst = storage.create_folder(None, "copy_dst").await.unwrap();
    let move_dst = storage.create_folder(None, "move_dst").await.unwrap();

    // A file inside src.
    let file = upload(storage, Some(src), "a.txt", b"hello world").await;

    // Rename it in place.
    storage.rename(file, "renamed.txt").await.unwrap();
    let info = storage.get_object_info(file).await.unwrap();
    assert_eq!(info.filename, "renamed.txt");

    // Copy it into copy_dst (a NEW handle, original stays under src).
    let copy = storage.copy_object(file, copy_dst, None).await.unwrap();
    assert_ne!(copy, file);
    let copy_info = storage.get_object_info(copy).await.unwrap();
    assert_eq!(copy_info.filename, "renamed.txt");
    assert_eq!(copy_info.parent, copy_dst);
    assert_eq!(storage.get_object_info(file).await.unwrap().parent, src);

    // Move the original into move_dst.
    storage.move_object(file, move_dst, None).await.unwrap();
    assert_eq!(
        storage.get_object_info(file).await.unwrap().parent,
        move_dst
    );

    // The copy is untouched under copy_dst; the moved original is the lone child of move_dst.
    let in_copy = storage.list_objects(Some(copy_dst)).await.unwrap();
    assert_eq!(in_copy.len(), 1, "copy_dst holds the copy");
    let in_move = storage.list_objects(Some(move_dst)).await.unwrap();
    assert_eq!(in_move.len(), 1, "move_dst holds the moved original");
    // src is now empty (the original moved out).
    assert!(storage.list_objects(Some(src)).await.unwrap().is_empty());
}

#[tokio::test]
async fn recursive_listing_walks_the_tree() {
    let (device, _dir) = open_device("conf-recursive").await;
    let storage = &device.storages().await.unwrap()[0];

    let top = storage.create_folder(None, "top").await.unwrap();
    let sub = storage.create_folder(Some(top), "sub").await.unwrap();
    upload(storage, Some(top), "top.txt", b"top").await;
    upload(storage, Some(sub), "deep.txt", b"deep").await;

    let all = storage.list_objects_recursive(Some(top)).await.unwrap();
    let names: Vec<&str> = all.iter().map(|o| o.filename.as_str()).collect();
    assert!(names.contains(&"sub"));
    assert!(names.contains(&"top.txt"));
    assert!(names.contains(&"deep.txt"));
}

#[tokio::test]
async fn upload_with_progress_reports_and_completes() {
    let (device, _dir) = open_device("conf-progress").await;
    let storage = &device.storages().await.unwrap()[0];

    let content = vec![7u8; 4096];
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_cb = Arc::clone(&calls);
    let info = NewObjectInfo::file("progress.bin", content.len() as u64);

    let handle = storage
        .upload_with_progress(None, info, upload_stream(&content), move |p| {
            calls_cb.fetch_add(1, Ordering::SeqCst);
            assert!(p.total_bytes == Some(content.len() as u64));
            std::ops::ControlFlow::Continue(())
        })
        .await
        .expect("upload should complete");

    assert!(calls.load(Ordering::SeqCst) >= 1, "progress should fire");
    assert_eq!(storage.download_to_vec(handle).await.unwrap().len(), 4096);
}

#[tokio::test]
async fn upload_cancel_surfaces_partial_handle() {
    let (device, _dir) = open_device("conf-upload-cancel").await;
    let storage = &device.storages().await.unwrap()[0];

    let info = NewObjectInfo::file("cancelled.bin", 5);
    let err = storage
        .upload_with_progress(None, info, upload_stream(b"hello"), |_| {
            std::ops::ControlFlow::Break(())
        })
        .await
        .expect_err("a cancelled upload should fail");

    assert!(matches!(err.source, Error::Cancelled));
    assert!(
        err.partial.is_some(),
        "the partial handle must be surfaced so the caller can clean up"
    );
    // And the caller can delete it with the surfaced handle.
    storage.delete(err.partial.unwrap()).await.unwrap();
}

#[tokio::test]
async fn download_cancel_leaves_session_usable() {
    let (device, _dir) = open_device("conf-dl-cancel").await;
    let storage = &device.storages().await.unwrap()[0];

    let content = vec![3u8; 64 * 1024];
    let handle = upload(storage, None, "big.bin", &content).await;

    // Start a streaming download, pull one chunk, then cancel and drop. The
    // download holds the session for the whole transfer, so it must be dropped
    // (here, end of scope) after cancel() before the session is free again — the
    // documented contract.
    {
        let mut dl = storage.download(handle, ByteRange::Full).await.unwrap();
        let first = dl.next_chunk().await.expect("at least one chunk").unwrap();
        assert!(!first.is_empty());
        dl.cancel(Duration::from_millis(300)).await.unwrap();
    }

    // The session is healthy for the next operation (cancel drained the pipe).
    let listed = storage.list_objects(None).await.unwrap();
    assert!(listed.iter().any(|o| o.filename == "big.bin"));
}

#[tokio::test]
async fn download_drop_leaves_session_usable() {
    let (device, _dir) = open_device("conf-dl-drop").await;
    let storage = &device.storages().await.unwrap()[0];

    let content = vec![9u8; 64 * 1024];
    let handle = upload(storage, None, "drop.bin", &content).await;

    // Start a download, pull one chunk, then DROP it without cancelling. The session self-heals on
    // the next operation (lazy recovery).
    {
        let mut dl = storage.download(handle, ByteRange::Full).await.unwrap();
        let _first = dl.next_chunk().await.expect("at least one chunk").unwrap();
        // dl dropped here
    }

    let listed = storage.list_objects(None).await.unwrap();
    assert!(listed.iter().any(|o| o.filename == "drop.bin"));
}

#[tokio::test]
async fn list_cancel_token_bails() {
    use mtp_rs::CancelToken;

    let (device, _dir) = open_device("conf-list-cancel").await;
    let storage = &device.storages().await.unwrap()[0];

    for i in 0..5 {
        upload(storage, None, &format!("f{i}.txt"), b"x").await;
    }

    let cancel = CancelToken::new();
    cancel.cancel();
    let result = storage.list_objects_with_cancel(None, Some(&cancel)).await;
    assert!(matches!(result, Err(Error::Cancelled)));
}

#[tokio::test]
async fn thumbnail_unsupported_errors_gracefully() {
    let (device, _dir) = open_device("conf-thumb").await;
    let storage = &device.storages().await.unwrap()[0];

    let handle = upload(storage, None, "pic.jpg", b"not a real jpeg").await;
    // The virtual device has no thumbnails: it must surface an error, not panic or hang.
    let result = storage.thumbnail(handle).await;
    assert!(result.is_err(), "virtual device reports no thumbnail");
}
