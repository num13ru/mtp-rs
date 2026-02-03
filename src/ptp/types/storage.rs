//! Storage-related type enums for MTP/PTP.
//!
//! This module contains enums for describing storage characteristics:
//! - [`StorageType`]: Type of storage medium (ROM, RAM, etc.)
//! - [`FilesystemType`]: Type of filesystem on the storage
//! - [`AccessCapability`]: Read/write access capabilities
//! - [`ProtectionStatus`]: Object protection status
//! - [`AssociationType`]: Association type for objects (folder/container type)

// --- Storage Type Enum ---

/// Type of storage medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageType {
    /// Undefined storage type.
    #[default]
    Undefined,
    /// Fixed ROM (e.g., internal flash).
    FixedRom,
    /// Removable ROM.
    RemovableRom,
    /// Fixed RAM.
    FixedRam,
    /// Removable RAM (e.g., SD card).
    RemovableRam,
    /// Unknown storage type code.
    Unknown(u16),
}

impl StorageType {
    /// Convert a raw u16 code to a StorageType.
    #[must_use]
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => StorageType::Undefined,
            1 => StorageType::FixedRom,
            2 => StorageType::RemovableRom,
            3 => StorageType::FixedRam,
            4 => StorageType::RemovableRam,
            _ => StorageType::Unknown(code),
        }
    }

    /// Convert a StorageType to its raw u16 value.
    #[must_use]
    pub fn to_code(self) -> u16 {
        match self {
            StorageType::Undefined => 0,
            StorageType::FixedRom => 1,
            StorageType::RemovableRom => 2,
            StorageType::FixedRam => 3,
            StorageType::RemovableRam => 4,
            StorageType::Unknown(code) => code,
        }
    }
}

// --- Filesystem Type Enum ---

/// Type of filesystem on the storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilesystemType {
    /// Undefined filesystem type.
    #[default]
    Undefined,
    /// Generic flat filesystem (no folders).
    GenericFlat,
    /// Generic hierarchical filesystem (with folders).
    GenericHierarchical,
    /// DCF (Design rule for Camera File system).
    Dcf,
    /// Unknown filesystem type code.
    Unknown(u16),
}

impl FilesystemType {
    /// Convert a raw u16 code to a FilesystemType.
    #[must_use]
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => FilesystemType::Undefined,
            1 => FilesystemType::GenericFlat,
            2 => FilesystemType::GenericHierarchical,
            3 => FilesystemType::Dcf,
            _ => FilesystemType::Unknown(code),
        }
    }

    /// Convert a FilesystemType to its raw u16 value.
    #[must_use]
    pub fn to_code(self) -> u16 {
        match self {
            FilesystemType::Undefined => 0,
            FilesystemType::GenericFlat => 1,
            FilesystemType::GenericHierarchical => 2,
            FilesystemType::Dcf => 3,
            FilesystemType::Unknown(code) => code,
        }
    }
}

// --- Access Capability Enum ---

/// Access capability of the storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessCapability {
    /// Read-write access.
    #[default]
    ReadWrite,
    /// Read-only, deletion not allowed.
    ReadOnlyWithoutDeletion,
    /// Read-only, deletion allowed.
    ReadOnlyWithDeletion,
    /// Unknown access capability code.
    Unknown(u16),
}

impl AccessCapability {
    /// Convert a raw u16 code to an AccessCapability.
    #[must_use]
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => AccessCapability::ReadWrite,
            1 => AccessCapability::ReadOnlyWithoutDeletion,
            2 => AccessCapability::ReadOnlyWithDeletion,
            _ => AccessCapability::Unknown(code),
        }
    }

    /// Convert an AccessCapability to its raw u16 value.
    #[must_use]
    pub fn to_code(self) -> u16 {
        match self {
            AccessCapability::ReadWrite => 0,
            AccessCapability::ReadOnlyWithoutDeletion => 1,
            AccessCapability::ReadOnlyWithDeletion => 2,
            AccessCapability::Unknown(code) => code,
        }
    }
}

// --- Protection Status Enum ---

/// Protection status of an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtectionStatus {
    /// No protection.
    #[default]
    None,
    /// Read-only protection.
    ReadOnly,
    /// Unknown protection status code.
    Unknown(u16),
}

impl ProtectionStatus {
    /// Convert a raw u16 code to a ProtectionStatus.
    #[must_use]
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => ProtectionStatus::None,
            1 => ProtectionStatus::ReadOnly,
            _ => ProtectionStatus::Unknown(code),
        }
    }

    /// Convert a ProtectionStatus to its raw u16 value.
    #[must_use]
    pub fn to_code(self) -> u16 {
        match self {
            ProtectionStatus::None => 0,
            ProtectionStatus::ReadOnly => 1,
            ProtectionStatus::Unknown(code) => code,
        }
    }
}

// --- Association Type Enum ---

/// Association type for objects (folder/container type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssociationType {
    /// No association (regular file).
    #[default]
    None,
    /// Generic folder.
    GenericFolder,
    /// Unknown association type code.
    Unknown(u16),
}

impl AssociationType {
    /// Convert a raw u16 code to an AssociationType.
    #[must_use]
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => AssociationType::None,
            1 => AssociationType::GenericFolder,
            _ => AssociationType::Unknown(code),
        }
    }

    /// Convert an AssociationType to its raw u16 value.
    #[must_use]
    pub fn to_code(self) -> u16 {
        match self {
            AssociationType::None => 0,
            AssociationType::GenericFolder => 1,
            AssociationType::Unknown(code) => code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn storage_type_conversions() {
        for (code, expected) in [
            (0, StorageType::Undefined),
            (1, StorageType::FixedRom),
            (2, StorageType::RemovableRom),
            (3, StorageType::FixedRam),
            (4, StorageType::RemovableRam),
        ] {
            assert_eq!(StorageType::from_code(code), expected);
            assert_eq!(expected.to_code(), code);
        }
        assert_eq!(StorageType::from_code(99), StorageType::Unknown(99));
        assert_eq!(StorageType::default(), StorageType::Undefined);
    }

    #[test]
    fn filesystem_type_conversions() {
        for (code, expected) in [
            (0, FilesystemType::Undefined),
            (1, FilesystemType::GenericFlat),
            (2, FilesystemType::GenericHierarchical),
            (3, FilesystemType::Dcf),
        ] {
            assert_eq!(FilesystemType::from_code(code), expected);
            assert_eq!(expected.to_code(), code);
        }
        assert_eq!(FilesystemType::from_code(99), FilesystemType::Unknown(99));
        assert_eq!(FilesystemType::default(), FilesystemType::Undefined);
    }

    #[test]
    fn access_capability_conversions() {
        for (code, expected) in [
            (0, AccessCapability::ReadWrite),
            (1, AccessCapability::ReadOnlyWithoutDeletion),
            (2, AccessCapability::ReadOnlyWithDeletion),
        ] {
            assert_eq!(AccessCapability::from_code(code), expected);
            assert_eq!(expected.to_code(), code);
        }
        assert_eq!(AccessCapability::from_code(99), AccessCapability::Unknown(99));
        assert_eq!(AccessCapability::default(), AccessCapability::ReadWrite);
    }

    #[test]
    fn protection_status_conversions() {
        for (code, expected) in [(0, ProtectionStatus::None), (1, ProtectionStatus::ReadOnly)] {
            assert_eq!(ProtectionStatus::from_code(code), expected);
            assert_eq!(expected.to_code(), code);
        }
        assert_eq!(ProtectionStatus::from_code(99), ProtectionStatus::Unknown(99));
        assert_eq!(ProtectionStatus::default(), ProtectionStatus::None);
    }

    #[test]
    fn association_type_conversions() {
        for (code, expected) in [(0, AssociationType::None), (1, AssociationType::GenericFolder)] {
            assert_eq!(AssociationType::from_code(code), expected);
            assert_eq!(expected.to_code(), code);
        }
        assert_eq!(AssociationType::from_code(99), AssociationType::Unknown(99));
        assert_eq!(AssociationType::default(), AssociationType::None);
    }

    proptest! {
        #[test]
        fn prop_storage_type_roundtrip(code: u16) {
            let st = StorageType::from_code(code);
            prop_assert_eq!(st.to_code(), code);
            if code > 4 {
                prop_assert_eq!(st, StorageType::Unknown(code));
            }
        }

        #[test]
        fn prop_filesystem_type_roundtrip(code: u16) {
            let ft = FilesystemType::from_code(code);
            prop_assert_eq!(ft.to_code(), code);
            if code > 3 {
                prop_assert_eq!(ft, FilesystemType::Unknown(code));
            }
        }

        #[test]
        fn prop_access_capability_roundtrip(code: u16) {
            let ac = AccessCapability::from_code(code);
            prop_assert_eq!(ac.to_code(), code);
            if code > 2 {
                prop_assert_eq!(ac, AccessCapability::Unknown(code));
            }
        }

        #[test]
        fn prop_protection_status_roundtrip(code: u16) {
            let ps = ProtectionStatus::from_code(code);
            prop_assert_eq!(ps.to_code(), code);
            if code > 1 {
                prop_assert_eq!(ps, ProtectionStatus::Unknown(code));
            }
        }

        #[test]
        fn prop_association_type_roundtrip(code: u16) {
            let at = AssociationType::from_code(code);
            prop_assert_eq!(at.to_code(), code);
            if code > 1 {
                prop_assert_eq!(at, AssociationType::Unknown(code));
            }
        }
    }
}
