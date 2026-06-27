//! Backend-conformance suite for the **Windows WPD backend against a real device**.
//!
//! These run the exact same backend-neutral assertions as `conformance.rs` (the `common::run_*`
//! harness), but pointed at a physical phone over `Backend::Wpd` instead of the virtual device. They
//! parameterize the two backend divergences accordingly:
//!
//! - **Writable parent**: real Android/WPD rejects uploads to the storage root, so every test works
//!   inside a uniquely-named scoped folder created under `Download` (passed as `parent = Some(..)`).
//! - **Upload-cancel partial handle**: WPD commits transactionally, so a cancelled upload leaves no
//!   partial object (`expects_partial = false`), unlike the two-phase USB backend.
//!
//! Each test is `#[ignore]` so a normal `cargo test` run (CI without a phone) skips it; run them
//! locally with a phone connected in MTP/File-transfer mode, screen unlocked:
//!
//! ```text
//! cargo test -p mtp-rs --test conformance_wpd -- --ignored --nocapture
//! ```
//!
//! Every test creates its own scoped folder and deletes it on the way out — on success *or* panic
//! (via `catch_unwind`) — so your real files are never touched and nothing leaks between runs.

#![cfg(windows)]

mod common;

use futures::FutureExt;
use mtp_rs::{Backend, MtpDevice, ObjectHandle, Storage};
use std::panic::AssertUnwindSafe;

/// Open the real device over WPD, find `Download`, and create a fresh scoped test folder under it.
///
/// Returns the open device (kept alive by the caller), the owned [`Storage`], and the handle of the
/// scoped folder to run the harness inside.
async fn open_scoped(folder: &str) -> (MtpDevice, Storage, ObjectHandle) {
    let device = MtpDevice::builder()
        .backend(Backend::Wpd)
        .open_first()
        .await
        .expect("open a WPD device (phone connected, unlocked, MTP mode)");

    let storage = device
        .storages()
        .await
        .expect("list storages")
        .into_iter()
        .next()
        .expect("device has at least one storage");

    // Android rejects uploads to the storage root, so work inside Download.
    let root = storage.list_objects(None).await.expect("list storage root");
    let download = root
        .iter()
        .find(|o| o.is_folder() && o.filename.eq_ignore_ascii_case("Download"))
        .expect("a Download folder in the storage root")
        .handle;

    let test_dir = storage
        .create_folder(Some(download), folder)
        .await
        .expect("create the scoped test folder under Download");
    (device, storage, test_dir)
}

/// Generate an `#[ignore]`d WPD conformance test that:
/// 1. opens the device and creates a uniquely-named scoped folder under `Download`,
/// 2. runs `$body(&storage, scoped_folder_handle)` (the shared harness fn), and
/// 3. **always** deletes the scoped folder afterwards — even if the body panicked.
macro_rules! wpd_conformance {
    ($name:ident, $folder:expr, $body:expr) => {
        #[tokio::test]
        #[ignore = "requires a real WPD device connected in MTP mode"]
        async fn $name() {
            let (_device, storage, dir) = open_scoped($folder).await;

            // Run the shared harness, catching a panic so the scoped folder is always cleaned up.
            let result = AssertUnwindSafe(($body)(&storage, dir))
                .catch_unwind()
                .await;

            // Cleanup: remove the scoped folder (and everything left inside it) on every exit.
            if let Err(e) = storage.delete(dir).await {
                eprintln!(
                    "cleanup: failed to delete scoped folder {dir:?}: {e} — remove it manually"
                );
            }

            if let Err(panic) = result {
                std::panic::resume_unwind(panic);
            }
        }
    };
}

wpd_conformance!(wpd_round_trip, "mtp-rs-conformance-roundtrip", |s, dir| {
    common::run_round_trip(s, Some(dir))
});

wpd_conformance!(
    wpd_ranged_and_resumable,
    "mtp-rs-conformance-ranged",
    |s, dir| { common::run_ranged_and_resumable(s, Some(dir)) }
);

wpd_conformance!(
    wpd_windowed_download,
    "mtp-rs-conformance-windowed",
    |s, dir| { common::run_windowed_download(s, Some(dir)) }
);

wpd_conformance!(
    wpd_create_folder_rename_move_copy,
    "mtp-rs-conformance-fsops",
    |s, dir| common::run_create_folder_rename_move_copy(s, Some(dir))
);

wpd_conformance!(
    wpd_recursive_listing,
    "mtp-rs-conformance-recursive",
    |s, dir| { common::run_recursive_listing(s, Some(dir)) }
);

wpd_conformance!(
    wpd_upload_with_progress,
    "mtp-rs-conformance-progress",
    |s, dir| { common::run_upload_with_progress(s, Some(dir)) }
);

// WPD commits transactionally: a cancelled upload leaves no partial handle (`expects_partial = false`).
wpd_conformance!(
    wpd_upload_cancel,
    "mtp-rs-conformance-upload-cancel",
    |s, dir| { common::run_upload_cancel(s, Some(dir), false) }
);

wpd_conformance!(
    wpd_download_cancel,
    "mtp-rs-conformance-dl-cancel",
    |s, dir| { common::run_download_cancel(s, Some(dir)) }
);

wpd_conformance!(wpd_download_drop, "mtp-rs-conformance-dl-drop", |s, dir| {
    common::run_download_drop(s, Some(dir))
});

wpd_conformance!(
    wpd_list_cancel_token,
    "mtp-rs-conformance-list-cancel",
    |s, dir| { common::run_list_cancel_token(s, Some(dir)) }
);

wpd_conformance!(
    wpd_thumbnail_unsupported,
    "mtp-rs-conformance-thumb",
    |s, dir| { common::run_thumbnail_unsupported(s, Some(dir)) }
);
