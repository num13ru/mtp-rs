//! Helpers shared across subcommands.
//!
//! Anything used in more than one command file lives here so the per-command
//! files stay narrow. Command-specific helpers (like `put`'s file-stream
//! adapter) stay in their own file.

use mtp_rs::mtp::Storage;
use mtp_rs::ptp::ObjectInfo;
use mtp_rs::{MtpDevice, ObjectHandle};

use super::error::{CliError, CliErrorKind};
use super::path::{self, ExistingRemote, RemotePath, UploadTarget};

pub async fn folder_parent(
    storage: &Storage,
    path: &RemotePath,
    verbose: bool,
) -> Result<(Option<ObjectHandle>, String), CliError> {
    match path::resolve_existing(storage, path, verbose).await? {
        ExistingRemote::Root => Ok((None, "/".to_string())),
        ExistingRemote::Object(object) if object.is_folder() => {
            Ok((Some(object.handle), path.raw().to_string()))
        }
        ExistingRemote::Object(_) => Err(CliError::new(
            CliErrorKind::RemotePath,
            "remote path is not a folder",
        )),
    }
}

pub fn parent_path_string(components: &[String]) -> String {
    if components.len() <= 1 {
        "/".to_string()
    } else {
        format!("/{}", components[..components.len() - 1].join("/"))
    }
}

pub async fn existing_object(
    storage: &Storage,
    path: &RemotePath,
    verbose: bool,
) -> Result<ObjectInfo, CliError> {
    match path::resolve_existing(storage, path, verbose).await? {
        ExistingRemote::Root => Err(CliError::new(
            CliErrorKind::RemotePath,
            "remote path cannot be the storage root",
        )),
        ExistingRemote::Object(object) => Ok(object),
    }
}

pub async fn resolve_object_destination(
    storage: &Storage,
    destination_path: &RemotePath,
    source_filename: &str,
    verbose: bool,
) -> Result<UploadTarget, CliError> {
    path::resolve_upload_target(storage, destination_path, source_filename, verbose).await
}

pub fn ensure_rename_supported(device: &MtpDevice) -> Result<(), CliError> {
    if device.supports_rename() {
        Ok(())
    } else {
        Err(CliError::new(
            CliErrorKind::Transfer,
            "device does not support rename; cannot change the destination filename",
        ))
    }
}

pub async fn ensure_rename_target_available(
    storage: &Storage,
    source_path: &RemotePath,
    source_handle: ObjectHandle,
    new_name: &str,
    verbose: bool,
) -> Result<(), CliError> {
    let parent_path = RemotePath::parse(&parent_path_string(source_path.components()))?;
    let parent = match path::resolve_existing(storage, &parent_path, verbose).await? {
        ExistingRemote::Root => None,
        ExistingRemote::Object(object) if object.is_folder() => Some(object.handle),
        ExistingRemote::Object(_) => {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "remote parent is not a folder",
            ));
        }
    };
    let siblings = storage
        .list_objects(parent)
        .await
        .map_err(|e| CliError::from_mtp("list remote parent", e, verbose))?;

    if siblings
        .iter()
        .any(|object| object.filename == new_name && object.handle != source_handle)
    {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            "destination name already exists",
        ));
    }

    Ok(())
}

pub fn ensure_not_same_object(
    source: ObjectHandle,
    destination: ObjectHandle,
) -> Result<(), CliError> {
    if source == destination {
        Err(CliError::new(
            CliErrorKind::RemotePath,
            "source and destination are the same remote object",
        ))
    } else {
        Ok(())
    }
}
