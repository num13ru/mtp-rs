//! Internal state for the virtual MTP device.

use super::config::{VirtualDeviceConfig, VirtualStorageConfig};
use crate::ptp::{ObjectHandle, StorageId};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

/// Per-storage state.
#[derive(Debug)]
pub(super) struct VirtualStorageState {
    pub config: VirtualStorageConfig,
    pub storage_id: StorageId,
}

/// A tracked virtual object (file or directory).
#[derive(Debug, Clone)]
pub(super) struct VirtualObject {
    /// Path relative to the storage backing dir.
    pub rel_path: PathBuf,
    /// Which storage this object belongs to.
    pub storage_id: StorageId,
    /// Parent handle (`ObjectHandle::ROOT` for root-level objects).
    pub parent: ObjectHandle,
}

/// Pending SendObjectInfo state (MTP requires SendObjectInfo before SendObject).
#[derive(Debug, Clone)]
pub(super) struct PendingSendInfo {
    pub storage_id: StorageId,
    pub parent: ObjectHandle,
    pub filename: String,
    #[allow(dead_code)] // Part of the protocol state, kept for debugging
    pub size: u64,
    pub is_folder: bool,
    pub assigned_handle: ObjectHandle,
}

/// A command waiting for its data phase from the host.
///
/// When the host sends a command that expects a data phase (SendObjectInfo,
/// SendObject, SetObjectPropValue), the command is stored here. The next
/// `send_bulk` (data container) takes it and dispatches both together.
#[derive(Debug)]
pub(super) struct PendingCommand {
    pub code: u16,
    pub tx_id: u32,
    pub params: Vec<u32>,
}

/// Full mutable state of the virtual device.
#[derive(Debug)]
pub(super) struct VirtualDeviceState {
    pub config: VirtualDeviceConfig,
    pub session_open: bool,
    pub next_handle: u32,
    pub objects: HashMap<u32, VirtualObject>,
    pub storages: Vec<VirtualStorageState>,
    pub pending_send: Option<PendingSendInfo>,
    pub pending_command: Option<PendingCommand>,
    pub event_queue: VecDeque<Vec<u8>>,
    pub response_queue: VecDeque<Vec<u8>>,
}

impl VirtualDeviceState {
    /// Create initial state from config.
    pub fn new(config: VirtualDeviceConfig) -> Self {
        let storages: Vec<VirtualStorageState> = config
            .storages
            .iter()
            .enumerate()
            .map(|(i, sc)| {
                // Canonicalize backing dirs so that all paths (from handlers, watcher,
                // dedup tracker) use the same form. On macOS, /var → /private/var.
                let mut resolved_config = sc.clone();
                if let Ok(canonical) = sc.backing_dir.canonicalize() {
                    resolved_config.backing_dir = canonical;
                }
                VirtualStorageState {
                    config: resolved_config,
                    // Storage IDs conventionally start at 0x00010001
                    storage_id: StorageId(0x0001_0001 + i as u32),
                }
            })
            .collect();

        Self {
            config,
            session_open: false,
            next_handle: 1,
            objects: HashMap::new(),
            storages,
            pending_send: None,
            pending_command: None,
            event_queue: VecDeque::new(),
            response_queue: VecDeque::new(),
        }
    }

    /// Allocate the next object handle.
    pub fn alloc_handle(&mut self) -> ObjectHandle {
        let h = self.next_handle;
        self.next_handle += 1;
        ObjectHandle(h)
    }

    /// Find a storage state by ID.
    pub fn find_storage(&self, id: StorageId) -> Option<&VirtualStorageState> {
        self.storages.iter().find(|s| s.storage_id == id)
    }

    /// Find or create handles for all entries in a directory.
    /// Returns handles for direct children of `parent` within the given storage.
    pub fn scan_dir(
        &mut self,
        storage_id: StorageId,
        parent: ObjectHandle,
    ) -> Result<Vec<ObjectHandle>, std::io::Error> {
        let storage = self
            .storages
            .iter()
            .find(|s| s.storage_id == storage_id)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "storage not found")
            })?;

        let base_dir = &storage.config.backing_dir;

        // Determine the filesystem path for this parent
        let dir_path = if parent == ObjectHandle::ROOT || parent.0 == 0 {
            base_dir.clone()
        } else {
            match self.objects.get(&parent.0) {
                Some(obj) if obj.storage_id == storage_id => base_dir.join(&obj.rel_path),
                _ => return Ok(Vec::new()),
            }
        };

        if !dir_path.is_dir() {
            return Ok(Vec::new());
        }

        let mut handles = Vec::new();
        let entries = std::fs::read_dir(&dir_path)?;

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            let rel = if parent == ObjectHandle::ROOT || parent.0 == 0 {
                PathBuf::from(&file_name)
            } else {
                let parent_obj = self.objects.get(&parent.0).unwrap();
                parent_obj.rel_path.join(&file_name)
            };

            // Check if we already have a handle for this path in this storage
            let existing = self
                .objects
                .iter()
                .find(|(_, obj)| obj.storage_id == storage_id && obj.rel_path == rel);

            let handle = if let Some((&h, _)) = existing {
                ObjectHandle(h)
            } else {
                let h = self.alloc_handle();
                self.objects.insert(
                    h.0,
                    VirtualObject {
                        rel_path: rel,
                        storage_id,
                        parent,
                    },
                );
                h
            };

            handles.push(handle);
        }

        Ok(handles)
    }

    /// Recursively scan all objects in a storage.
    pub fn scan_all(&mut self, storage_id: StorageId) -> Result<Vec<ObjectHandle>, std::io::Error> {
        // First scan root
        let root_handles = self.scan_dir(storage_id, ObjectHandle::ROOT)?;
        let mut all_handles = root_handles.clone();

        // BFS through directories
        let mut queue = root_handles;
        while let Some(handle) = queue.pop() {
            let obj = match self.objects.get(&handle.0) {
                Some(o) => o.clone(),
                None => continue,
            };
            let storage = match self.find_storage(storage_id) {
                Some(s) => s,
                None => continue,
            };
            let full_path = storage.config.backing_dir.join(&obj.rel_path);
            if full_path.is_dir() {
                let children = self.scan_dir(storage_id, handle)?;
                all_handles.extend(&children);
                queue.extend(children);
            }
        }

        Ok(all_handles)
    }

    /// Resolve the full filesystem path for an object handle.
    pub fn resolve_path(&self, handle: ObjectHandle) -> Option<PathBuf> {
        let obj = self.objects.get(&handle.0)?;
        let storage = self.find_storage(obj.storage_id)?;
        Some(storage.config.backing_dir.join(&obj.rel_path))
    }

    /// Check if a storage is read-only.
    pub fn is_read_only(&self, storage_id: StorageId) -> bool {
        self.find_storage(storage_id)
            .map(|s| s.config.read_only)
            .unwrap_or(false)
    }
}
