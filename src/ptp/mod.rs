//! Low-level PTP (Picture Transfer Protocol) implementation.
//!
//! This module provides the protocol-level types and functions for MTP/PTP communication.
//! Most users should prefer the high-level `mtp` module instead.
//!
//! ## Module structure
//!
//! - `codes`: Operation, response, event, and format code enums
//! - `pack`: Binary serialization/deserialization primitives
//! - `container`: USB container format (Phase 2)
//! - `types`: DeviceInfo, StorageInfo, ObjectInfo structures (Phase 2)
//! - `session`: PTP session management (Phase 4)
//! - `device`: PtpDevice public API (Phase 5)

mod codes;
mod container;
mod pack;
mod types;
// mod session;   // Phase 4
// mod device;    // Phase 5

pub use codes::{EventCode, ObjectFormatCode, OperationCode, ResponseCode};
pub use container::{
    container_type, CommandContainer, ContainerType, DataContainer, EventContainer,
    ResponseContainer,
};
pub use pack::{
    pack_datetime, pack_string, pack_u16, pack_u16_array, pack_u32, pack_u32_array, pack_u64,
    pack_u8, unpack_datetime, unpack_string, unpack_u16, unpack_u16_array, unpack_u32,
    unpack_u32_array, unpack_u64, unpack_u8, DateTime,
};
pub use types::{
    AccessCapability, AssociationType, DeviceInfo, FilesystemType, ObjectInfo, ProtectionStatus,
    StorageInfo, StorageType,
};

/// 32-bit object handle assigned by the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ObjectHandle(pub u32);

impl ObjectHandle {
    /// Root folder (parent = root means object is in storage root).
    pub const ROOT: Self = ObjectHandle(0x00000000);
    /// All objects (used in GetObjectHandles to list recursively).
    pub const ALL: Self = ObjectHandle(0xFFFFFFFF);
}

/// 32-bit storage identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StorageId(pub u32);

impl StorageId {
    /// All storages (used in GetObjectHandles to search all).
    pub const ALL: Self = StorageId(0xFFFFFFFF);
}

/// 32-bit session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SessionId(pub u32);

/// 32-bit transaction identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransactionId(pub u32);

impl TransactionId {
    /// The first valid transaction ID in a session.
    pub const FIRST: Self = TransactionId(0x00000001);

    /// Invalid transaction ID (must never be used).
    pub const INVALID: Self = TransactionId(0xFFFFFFFF);

    /// Transaction ID for session-less operations (e.g., GetDeviceInfo before OpenSession).
    pub const SESSION_LESS: Self = TransactionId(0x00000000);

    /// Get the next transaction ID, wrapping correctly.
    ///
    /// Wraps from 0xFFFFFFFE to 0x00000001 (skipping both 0x00000000 and 0xFFFFFFFF).
    pub fn next(self) -> Self {
        let next = self.0.wrapping_add(1);
        if next == 0 || next == 0xFFFFFFFF {
            TransactionId(0x00000001)
        } else {
            TransactionId(next)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_id_next() {
        assert_eq!(TransactionId(1).next(), TransactionId(2));
        assert_eq!(TransactionId(100).next(), TransactionId(101));
    }

    #[test]
    fn transaction_id_wrapping() {
        // Should wrap from 0xFFFFFFFE to 0x00000001, skipping 0xFFFFFFFF and 0x00000000
        assert_eq!(TransactionId(0xFFFFFFFE).next(), TransactionId(1));
        assert_eq!(TransactionId(0xFFFFFFFD).next(), TransactionId(0xFFFFFFFE));
    }

    #[test]
    fn object_handle_constants() {
        assert_eq!(ObjectHandle::ROOT.0, 0);
        assert_eq!(ObjectHandle::ALL.0, 0xFFFFFFFF);
    }

    #[test]
    fn storage_id_constants() {
        assert_eq!(StorageId::ALL.0, 0xFFFFFFFF);
    }
}
