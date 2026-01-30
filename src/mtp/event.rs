//! Device events.

use crate::ptp::{EventCode, EventContainer, ObjectHandle, StorageId};

/// Events from an MTP device.
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// A new object was added.
    ObjectAdded {
        /// Handle of the new object.
        handle: ObjectHandle,
    },

    /// An object was removed.
    ObjectRemoved {
        /// Handle of the removed object.
        handle: ObjectHandle,
    },

    /// A storage was added (e.g., SD card inserted).
    StoreAdded {
        /// ID of the new storage.
        storage_id: StorageId,
    },

    /// A storage was removed.
    StoreRemoved {
        /// ID of the removed storage.
        storage_id: StorageId,
    },

    /// Storage info changed (e.g., free space).
    StorageInfoChanged {
        /// ID of the storage that changed.
        storage_id: StorageId,
    },

    /// Object info changed.
    ObjectInfoChanged {
        /// Handle of the object that changed.
        handle: ObjectHandle,
    },

    /// Device info changed.
    DeviceInfoChanged,

    /// Device is being reset.
    DeviceReset,

    /// Unknown event.
    Unknown {
        /// Raw event code.
        code: u16,
        /// Event parameters.
        params: [u32; 3],
    },
}

impl DeviceEvent {
    /// Parse from an event container.
    pub fn from_container(container: &EventContainer) -> Self {
        match container.code {
            EventCode::ObjectAdded => DeviceEvent::ObjectAdded {
                handle: ObjectHandle(container.params[0]),
            },
            EventCode::ObjectRemoved => DeviceEvent::ObjectRemoved {
                handle: ObjectHandle(container.params[0]),
            },
            EventCode::StoreAdded => DeviceEvent::StoreAdded {
                storage_id: StorageId(container.params[0]),
            },
            EventCode::StoreRemoved => DeviceEvent::StoreRemoved {
                storage_id: StorageId(container.params[0]),
            },
            EventCode::StorageInfoChanged => DeviceEvent::StorageInfoChanged {
                storage_id: StorageId(container.params[0]),
            },
            EventCode::ObjectInfoChanged => DeviceEvent::ObjectInfoChanged {
                handle: ObjectHandle(container.params[0]),
            },
            EventCode::DeviceInfoChanged => DeviceEvent::DeviceInfoChanged,
            EventCode::Unknown(code) => DeviceEvent::Unknown {
                code,
                params: container.params,
            },
            _ => DeviceEvent::Unknown {
                code: container.code.to_code(),
                params: container.params,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_added_event() {
        let container = EventContainer {
            code: EventCode::ObjectAdded,
            transaction_id: 0,
            params: [42, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::ObjectAdded { handle } => {
                assert_eq!(handle, ObjectHandle(42));
            }
            _ => panic!("Expected ObjectAdded event"),
        }
    }

    #[test]
    fn test_object_removed_event() {
        let container = EventContainer {
            code: EventCode::ObjectRemoved,
            transaction_id: 0,
            params: [123, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::ObjectRemoved { handle } => {
                assert_eq!(handle, ObjectHandle(123));
            }
            _ => panic!("Expected ObjectRemoved event"),
        }
    }

    #[test]
    fn test_store_added_event() {
        let container = EventContainer {
            code: EventCode::StoreAdded,
            transaction_id: 0,
            params: [0x00010001, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::StoreAdded { storage_id } => {
                assert_eq!(storage_id, StorageId(0x00010001));
            }
            _ => panic!("Expected StoreAdded event"),
        }
    }

    #[test]
    fn test_store_removed_event() {
        let container = EventContainer {
            code: EventCode::StoreRemoved,
            transaction_id: 0,
            params: [0x00010002, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::StoreRemoved { storage_id } => {
                assert_eq!(storage_id, StorageId(0x00010002));
            }
            _ => panic!("Expected StoreRemoved event"),
        }
    }

    #[test]
    fn test_storage_info_changed_event() {
        let container = EventContainer {
            code: EventCode::StorageInfoChanged,
            transaction_id: 5,
            params: [0x00010001, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::StorageInfoChanged { storage_id } => {
                assert_eq!(storage_id, StorageId(0x00010001));
            }
            _ => panic!("Expected StorageInfoChanged event"),
        }
    }

    #[test]
    fn test_object_info_changed_event() {
        let container = EventContainer {
            code: EventCode::ObjectInfoChanged,
            transaction_id: 0,
            params: [99, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::ObjectInfoChanged { handle } => {
                assert_eq!(handle, ObjectHandle(99));
            }
            _ => panic!("Expected ObjectInfoChanged event"),
        }
    }

    #[test]
    fn test_device_info_changed_event() {
        let container = EventContainer {
            code: EventCode::DeviceInfoChanged,
            transaction_id: 0,
            params: [0, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::DeviceInfoChanged => {}
            _ => panic!("Expected DeviceInfoChanged event"),
        }
    }

    #[test]
    fn test_unknown_event() {
        let container = EventContainer {
            code: EventCode::Unknown(0x9999),
            transaction_id: 0,
            params: [1, 2, 3],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::Unknown { code, params } => {
                assert_eq!(code, 0x9999);
                assert_eq!(params, [1, 2, 3]);
            }
            _ => panic!("Expected Unknown event"),
        }
    }

    #[test]
    fn test_device_prop_changed_becomes_unknown() {
        // DevicePropChanged is a known EventCode but not a known DeviceEvent variant
        let container = EventContainer {
            code: EventCode::DevicePropChanged,
            transaction_id: 0,
            params: [100, 0, 0],
        };
        let event = DeviceEvent::from_container(&container);
        match event {
            DeviceEvent::Unknown { code, params } => {
                assert_eq!(code, 0x4006); // DevicePropChanged code
                assert_eq!(params[0], 100);
            }
            _ => panic!("Expected Unknown event for DevicePropChanged"),
        }
    }
}
