//! Filesystem watcher for detecting out-of-band changes to backing directories.
//!
//! When files are written directly to a virtual device's backing directory (bypassing
//! MTP protocol operations), this watcher detects the changes and queues the
//! corresponding MTP events (`ObjectAdded`, `ObjectRemoved`).
//!
//! ## Architecture
//!
//! The watcher callback runs on a background thread (macOS FSEvents / Linux inotify).
//! To avoid cross-thread timing issues with dedup, the callback does NOT process
//! events directly. Instead, it pushes lightweight `PendingFsEvent` entries into a
//! separate queue. The `receive_interrupt` method drains this queue on the caller's
//! thread, under the main state mutex, where the dedup check (does a handle already
//! exist for this path?) runs in the same execution context as MTP handlers.
//!
//! ## Dedup
//!
//! MTP handlers insert/remove handles in `state.objects` during `send_bulk`.
//! When `receive_interrupt` processes a pending fs event, it checks `state.objects`
//! under the same mutex. Since both run through the same mutex (and `receive_interrupt`
//! always runs after the `send_bulk` that caused the filesystem change), the handle
//! is guaranteed to be present → the event is skipped.

use super::builders::build_event;
use super::state::VirtualDeviceState;
use crate::ptp::{EventCode, ObjectHandle, StorageId};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A filesystem event pending processing by `receive_interrupt`.
#[derive(Debug)]
pub(super) struct PendingFsEvent {
    /// Path relative to the storage's backing dir.
    pub rel_path: PathBuf,
    /// Which storage this event belongs to.
    pub storage_id: StorageId,
    /// `true` for creation, `false` for removal.
    pub is_create: bool,
}

/// Mapping from backing dir to its storage ID, used by the watcher callback.
type StorageMap = Vec<(PathBuf, StorageId)>;

/// Start a filesystem watcher for all backing directories.
///
/// Returns the watcher (must be kept alive) and the pending-event queue,
/// or `None` if starting fails.
pub(super) fn start_fs_watcher(
    state: &Arc<Mutex<VirtualDeviceState>>,
) -> Option<(RecommendedWatcher, Arc<Mutex<Vec<PendingFsEvent>>>)> {
    let pending = Arc::new(Mutex::new(Vec::<PendingFsEvent>::new()));
    let pending_clone = Arc::clone(&pending);

    // Build a map of backing_dir → storage_id for resolving events.
    let storage_map: StorageMap = {
        let s = state.lock().unwrap();
        s.storages
            .iter()
            .map(|ss| {
                let dir = ss
                    .config
                    .backing_dir
                    .canonicalize()
                    .unwrap_or_else(|_| ss.config.backing_dir.clone());
                (dir, ss.storage_id)
            })
            .collect()
    };

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            let event = match res {
                Ok(e) => e,
                Err(_) => return,
            };

            // Filter: only handle create and remove events.
            let is_create = matches!(
                event.kind,
                EventKind::Create(_)
                    | EventKind::Modify(notify::event::ModifyKind::Name(
                        notify::event::RenameMode::To
                    ))
            );
            let is_remove = matches!(
                event.kind,
                EventKind::Remove(_)
                    | EventKind::Modify(notify::event::ModifyKind::Name(
                        notify::event::RenameMode::From
                    ))
            );

            if !is_create && !is_remove {
                return;
            }

            // Resolve paths and buffer as pending events.
            // This does NOT touch the state mutex — just the lightweight pending queue.
            let mut pending = pending_clone.lock().unwrap();
            for path in &event.paths {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

                let (storage_id, backing_dir) =
                    match find_storage_for_path(&canonical, &storage_map) {
                        Some(v) => v,
                        None => continue,
                    };

                let rel_path = match canonical.strip_prefix(backing_dir) {
                    Ok(r) => r.to_path_buf(),
                    Err(_) => continue,
                };

                pending.push(PendingFsEvent {
                    rel_path,
                    storage_id,
                    is_create,
                });
            }
        },
        Config::default(),
    )
    .ok()?;

    // Watch all backing directories recursively.
    let state_lock = state.lock().unwrap();
    for storage in &state_lock.storages {
        let _ = watcher.watch(&storage.config.backing_dir, RecursiveMode::Recursive);
    }
    drop(state_lock);

    Some((watcher, pending))
}

/// Process all pending filesystem events, performing dedup against `state.objects`.
///
/// Called from `receive_interrupt` on the caller's thread, under the state mutex.
/// This ensures dedup checks run in the same execution context as MTP handlers.
pub(super) fn process_pending_fs_events(
    state: &mut VirtualDeviceState,
    pending: &Mutex<Vec<PendingFsEvent>>,
) {
    let events: Vec<PendingFsEvent> = {
        let mut queue = pending.lock().unwrap();
        queue.drain(..).collect()
    };

    for fs_event in events {
        if fs_event.is_create {
            // Dedup: if the MTP handler already created a handle for this path, skip.
            let already_known = state.objects.iter().any(|(_, obj)| {
                obj.storage_id == fs_event.storage_id && obj.rel_path == fs_event.rel_path
            });

            if already_known {
                continue;
            }

            // Determine the parent handle.
            let parent = if let Some(parent_rel) = fs_event.rel_path.parent() {
                if parent_rel == std::path::Path::new("") {
                    ObjectHandle::ROOT
                } else {
                    match state.objects.iter().find(|(_, obj)| {
                        obj.storage_id == fs_event.storage_id && obj.rel_path == parent_rel
                    }) {
                        Some((&h, _)) => ObjectHandle(h),
                        None => ObjectHandle::ROOT,
                    }
                }
            } else {
                ObjectHandle::ROOT
            };

            let handle = state.alloc_handle();
            state.objects.insert(
                handle.0,
                super::state::VirtualObject {
                    rel_path: fs_event.rel_path,
                    storage_id: fs_event.storage_id,
                    parent,
                },
            );

            state
                .event_queue
                .push_back(build_event(EventCode::ObjectAdded, &[handle.0]));
            state.event_queue.push_back(build_event(
                EventCode::StorageInfoChanged,
                &[fs_event.storage_id.0],
            ));
        } else {
            // Remove: find the handle and remove it. If gone, MTP handler already did it.
            let handle = state
                .objects
                .iter()
                .find(|(_, obj)| {
                    obj.storage_id == fs_event.storage_id && obj.rel_path == fs_event.rel_path
                })
                .map(|(&h, _)| h);

            if let Some(h) = handle {
                state.objects.remove(&h);
                state
                    .event_queue
                    .push_back(build_event(EventCode::ObjectRemoved, &[h]));
                state.event_queue.push_back(build_event(
                    EventCode::StorageInfoChanged,
                    &[fs_event.storage_id.0],
                ));
            }
        }
    }
}

/// Find which storage a path belongs to by checking if it's under a backing dir.
fn find_storage_for_path<'a>(
    path: &std::path::Path,
    storage_map: &'a StorageMap,
) -> Option<(StorageId, &'a PathBuf)> {
    storage_map
        .iter()
        .find(|(dir, _)| path.starts_with(dir))
        .map(|(dir, sid)| (*sid, dir))
}
