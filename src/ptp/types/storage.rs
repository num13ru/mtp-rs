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
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => AccessCapability::ReadWrite,
            1 => AccessCapability::ReadOnlyWithoutDeletion,
            2 => AccessCapability::ReadOnlyWithDeletion,
            _ => AccessCapability::Unknown(code),
        }
    }

    /// Convert an AccessCapability to its raw u16 value.
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
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => ProtectionStatus::None,
            1 => ProtectionStatus::ReadOnly,
            _ => ProtectionStatus::Unknown(code),
        }
    }

    /// Convert a ProtectionStatus to its raw u16 value.
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
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => AssociationType::None,
            1 => AssociationType::GenericFolder,
            _ => AssociationType::Unknown(code),
        }
    }

    /// Convert an AssociationType to its raw u16 value.
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

    // =========================================================================
    // StorageType Tests
    // =========================================================================

    #[test]
    fn storage_type_from_code() {
        assert_eq!(StorageType::from_code(0), StorageType::Undefined);
        assert_eq!(StorageType::from_code(1), StorageType::FixedRom);
        assert_eq!(StorageType::from_code(2), StorageType::RemovableRom);
        assert_eq!(StorageType::from_code(3), StorageType::FixedRam);
        assert_eq!(StorageType::from_code(4), StorageType::RemovableRam);
        assert_eq!(StorageType::from_code(99), StorageType::Unknown(99));
    }

    #[test]
    fn storage_type_to_code() {
        assert_eq!(StorageType::Undefined.to_code(), 0);
        assert_eq!(StorageType::FixedRom.to_code(), 1);
        assert_eq!(StorageType::RemovableRom.to_code(), 2);
        assert_eq!(StorageType::FixedRam.to_code(), 3);
        assert_eq!(StorageType::RemovableRam.to_code(), 4);
        assert_eq!(StorageType::Unknown(99).to_code(), 99);
    }

    #[test]
    fn storage_type_roundtrip() {
        let types = [
            StorageType::Undefined,
            StorageType::FixedRom,
            StorageType::RemovableRom,
            StorageType::FixedRam,
            StorageType::RemovableRam,
        ];
        for t in types {
            assert_eq!(StorageType::from_code(t.to_code()), t);
        }
    }

    #[test]
    fn storage_type_default() {
        assert_eq!(StorageType::default(), StorageType::Undefined);
    }

    // =========================================================================
    // FilesystemType Tests
    // =========================================================================

    #[test]
    fn filesystem_type_from_code() {
        assert_eq!(FilesystemType::from_code(0), FilesystemType::Undefined);
        assert_eq!(FilesystemType::from_code(1), FilesystemType::GenericFlat);
        assert_eq!(
            FilesystemType::from_code(2),
            FilesystemType::GenericHierarchical
        );
        assert_eq!(FilesystemType::from_code(3), FilesystemType::Dcf);
        assert_eq!(FilesystemType::from_code(99), FilesystemType::Unknown(99));
    }

    #[test]
    fn filesystem_type_to_code() {
        assert_eq!(FilesystemType::Undefined.to_code(), 0);
        assert_eq!(FilesystemType::GenericFlat.to_code(), 1);
        assert_eq!(FilesystemType::GenericHierarchical.to_code(), 2);
        assert_eq!(FilesystemType::Dcf.to_code(), 3);
        assert_eq!(FilesystemType::Unknown(99).to_code(), 99);
    }

    #[test]
    fn filesystem_type_roundtrip() {
        let types = [
            FilesystemType::Undefined,
            FilesystemType::GenericFlat,
            FilesystemType::GenericHierarchical,
            FilesystemType::Dcf,
        ];
        for t in types {
            assert_eq!(FilesystemType::from_code(t.to_code()), t);
        }
    }

    #[test]
    fn filesystem_type_default() {
        assert_eq!(FilesystemType::default(), FilesystemType::Undefined);
    }

    // =========================================================================
    // AccessCapability Tests
    // =========================================================================

    #[test]
    fn access_capability_from_code() {
        assert_eq!(AccessCapability::from_code(0), AccessCapability::ReadWrite);
        assert_eq!(
            AccessCapability::from_code(1),
            AccessCapability::ReadOnlyWithoutDeletion
        );
        assert_eq!(
            AccessCapability::from_code(2),
            AccessCapability::ReadOnlyWithDeletion
        );
        assert_eq!(
            AccessCapability::from_code(99),
            AccessCapability::Unknown(99)
        );
    }

    #[test]
    fn access_capability_to_code() {
        assert_eq!(AccessCapability::ReadWrite.to_code(), 0);
        assert_eq!(AccessCapability::ReadOnlyWithoutDeletion.to_code(), 1);
        assert_eq!(AccessCapability::ReadOnlyWithDeletion.to_code(), 2);
        assert_eq!(AccessCapability::Unknown(99).to_code(), 99);
    }

    #[test]
    fn access_capability_roundtrip() {
        let caps = [
            AccessCapability::ReadWrite,
            AccessCapability::ReadOnlyWithoutDeletion,
            AccessCapability::ReadOnlyWithDeletion,
        ];
        for c in caps {
            assert_eq!(AccessCapability::from_code(c.to_code()), c);
        }
    }

    #[test]
    fn access_capability_default() {
        assert_eq!(AccessCapability::default(), AccessCapability::ReadWrite);
    }

    // =========================================================================
    // ProtectionStatus Tests
    // =========================================================================

    #[test]
    fn protection_status_from_code() {
        assert_eq!(ProtectionStatus::from_code(0), ProtectionStatus::None);
        assert_eq!(ProtectionStatus::from_code(1), ProtectionStatus::ReadOnly);
        assert_eq!(
            ProtectionStatus::from_code(99),
            ProtectionStatus::Unknown(99)
        );
    }

    #[test]
    fn protection_status_to_code() {
        assert_eq!(ProtectionStatus::None.to_code(), 0);
        assert_eq!(ProtectionStatus::ReadOnly.to_code(), 1);
        assert_eq!(ProtectionStatus::Unknown(99).to_code(), 99);
    }

    #[test]
    fn protection_status_roundtrip() {
        let statuses = [ProtectionStatus::None, ProtectionStatus::ReadOnly];
        for s in statuses {
            assert_eq!(ProtectionStatus::from_code(s.to_code()), s);
        }
    }

    #[test]
    fn protection_status_default() {
        assert_eq!(ProtectionStatus::default(), ProtectionStatus::None);
    }

    // =========================================================================
    // AssociationType Tests
    // =========================================================================

    #[test]
    fn association_type_from_code() {
        assert_eq!(AssociationType::from_code(0), AssociationType::None);
        assert_eq!(
            AssociationType::from_code(1),
            AssociationType::GenericFolder
        );
        assert_eq!(AssociationType::from_code(99), AssociationType::Unknown(99));
    }

    #[test]
    fn association_type_to_code() {
        assert_eq!(AssociationType::None.to_code(), 0);
        assert_eq!(AssociationType::GenericFolder.to_code(), 1);
        assert_eq!(AssociationType::Unknown(99).to_code(), 99);
    }

    #[test]
    fn association_type_roundtrip() {
        let types = [AssociationType::None, AssociationType::GenericFolder];
        for t in types {
            assert_eq!(AssociationType::from_code(t.to_code()), t);
        }
    }

    #[test]
    fn association_type_default() {
        assert_eq!(AssociationType::default(), AssociationType::None);
    }

    // =========================================================================
    // Property-based tests (proptest)
    // =========================================================================

    use proptest::prelude::*;

    // -------------------------------------------------------------------------
    // StorageType property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Known StorageType variants roundtrip correctly
        #[test]
        fn prop_storage_type_known_roundtrip(code in 0u16..=4u16) {
            let storage_type = StorageType::from_code(code);
            prop_assert_eq!(storage_type.to_code(), code);
        }

        /// Unknown StorageType values preserve the original code
        #[test]
        fn prop_storage_type_unknown_preserves_code(code in 5u16..=u16::MAX) {
            let storage_type = StorageType::from_code(code);
            prop_assert_eq!(storage_type, StorageType::Unknown(code));
            prop_assert_eq!(storage_type.to_code(), code);
        }
    }

    // -------------------------------------------------------------------------
    // FilesystemType property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Known FilesystemType variants roundtrip correctly
        #[test]
        fn prop_filesystem_type_known_roundtrip(code in 0u16..=3u16) {
            let fs_type = FilesystemType::from_code(code);
            prop_assert_eq!(fs_type.to_code(), code);
        }

        /// Unknown FilesystemType values preserve the original code
        #[test]
        fn prop_filesystem_type_unknown_preserves_code(code in 4u16..=u16::MAX) {
            let fs_type = FilesystemType::from_code(code);
            prop_assert_eq!(fs_type, FilesystemType::Unknown(code));
            prop_assert_eq!(fs_type.to_code(), code);
        }
    }

    // -------------------------------------------------------------------------
    // AccessCapability property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Known AccessCapability variants roundtrip correctly
        #[test]
        fn prop_access_capability_known_roundtrip(code in 0u16..=2u16) {
            let cap = AccessCapability::from_code(code);
            prop_assert_eq!(cap.to_code(), code);
        }

        /// Unknown AccessCapability values preserve the original code
        #[test]
        fn prop_access_capability_unknown_preserves_code(code in 3u16..=u16::MAX) {
            let cap = AccessCapability::from_code(code);
            prop_assert_eq!(cap, AccessCapability::Unknown(code));
            prop_assert_eq!(cap.to_code(), code);
        }
    }

    // -------------------------------------------------------------------------
    // ProtectionStatus property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Known ProtectionStatus variants roundtrip correctly
        #[test]
        fn prop_protection_status_known_roundtrip(code in 0u16..=1u16) {
            let status = ProtectionStatus::from_code(code);
            prop_assert_eq!(status.to_code(), code);
        }

        /// Unknown ProtectionStatus values preserve the original code
        #[test]
        fn prop_protection_status_unknown_preserves_code(code in 2u16..=u16::MAX) {
            let status = ProtectionStatus::from_code(code);
            prop_assert_eq!(status, ProtectionStatus::Unknown(code));
            prop_assert_eq!(status.to_code(), code);
        }
    }

    // -------------------------------------------------------------------------
    // AssociationType property tests
    // -------------------------------------------------------------------------

    proptest! {
        /// Known AssociationType variants roundtrip correctly
        #[test]
        fn prop_association_type_known_roundtrip(code in 0u16..=1u16) {
            let assoc = AssociationType::from_code(code);
            prop_assert_eq!(assoc.to_code(), code);
        }

        /// Unknown AssociationType values preserve the original code
        #[test]
        fn prop_association_type_unknown_preserves_code(code in 2u16..=u16::MAX) {
            let assoc = AssociationType::from_code(code);
            prop_assert_eq!(assoc, AssociationType::Unknown(code));
            prop_assert_eq!(assoc.to_code(), code);
        }
    }
}
