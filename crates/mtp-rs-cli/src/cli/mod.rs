mod args;
mod error;
mod path;

use args::{Cli, Command};
use bytes::Bytes;
use clap::Parser;
use error::{CliError, CliErrorKind};
use futures::Stream;
use mtp_rs::mtp::{MtpDeviceInfo, Storage};
use mtp_rs::ptp::ObjectInfo;
use mtp_rs::{MtpDevice, NewObjectInfo, ObjectHandle, StorageId};
use path::{ExistingRemote, RemotePath};
use serde::Serialize;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CHUNK_SIZE: usize = 256 * 1024;

pub async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Devices => devices(&cli),
        Command::Info => info(&cli).await,
        Command::Ls(args) => ls(&cli, &args.remote_path, args.recursive).await,
        Command::Put(args) => {
            put(
                &cli,
                &args.local_path,
                &args.remote_path,
                args.replace,
                args.verify,
            )
            .await
        }
        Command::Get(args) => get(&cli, &args.remote_path, &args.local_path, args.replace).await,
        Command::Mkdir(args) => mkdir(&cli, &args.remote_path).await,
        Command::Rm(args) => rm(&cli, &args.remote_path, args.yes).await,
        Command::Rename(args) => rename(&cli, &args.remote_path, &args.new_name).await,
        Command::Mv(args) => {
            move_remote(
                &cli,
                &args.source_path,
                &args.destination_path,
                args.replace,
            )
            .await
        }
        Command::Cp(args) => {
            copy_remote(
                &cli,
                &args.source_path,
                &args.destination_path,
                args.replace,
            )
            .await
        }
        Command::Doctor => doctor(&cli).await,
    }
}

fn devices(cli: &Cli) -> Result<(), CliError> {
    let devices = MtpDevice::list_devices_with_known(&cli.known)
        .map_err(|e| CliError::from_mtp("list devices", e, cli.verbose))?;
    let rows: Vec<DeviceRow> = devices.iter().map(DeviceRow::from).collect();

    if cli.json {
        print_json(&rows)
    } else {
        if rows.is_empty() {
            println!("No MTP devices found");
            return Ok(());
        }
        for row in rows {
            println!(
                "{} {} {:04x}:{:04x} serial={} location={} speed={}",
                row.manufacturer.as_deref().unwrap_or("Unknown"),
                row.product.as_deref().unwrap_or("Unknown"),
                row.vendor_id,
                row.product_id,
                row.serial_number.as_deref().unwrap_or("-"),
                row.location,
                row.speed.as_deref().unwrap_or("unknown"),
            );
        }
        Ok(())
    }
}

async fn info(cli: &Cli) -> Result<(), CliError> {
    let device = open_selected_device(cli).await?;
    let storages = device
        .storages()
        .await
        .map_err(|e| CliError::from_mtp("list storages", e, cli.verbose))?;
    let row = InfoRow {
        manufacturer: device.device_info().manufacturer.clone(),
        model: device.device_info().model.clone(),
        serial_number: device.device_info().serial_number.clone(),
        device_version: device.device_info().device_version.clone(),
        supports_rename: device.supports_rename(),
        storages: storages
            .iter()
            .enumerate()
            .map(|(index, storage)| StorageRow::from_storage(index, storage))
            .collect(),
    };

    if cli.json {
        print_json(&row)
    } else {
        println!(
            "{} {} serial={} version={}",
            row.manufacturer, row.model, row.serial_number, row.device_version
        );
        println!("supports rename: {}", row.supports_rename);
        for storage in row.storages {
            println!(
                "[{}] id={} {} free={} capacity={} access={}",
                storage.index,
                storage.id,
                storage.description,
                storage.free_space_bytes,
                storage.max_capacity,
                storage.access_capability
            );
        }
        Ok(())
    }
}

async fn ls(cli: &Cli, remote_path: &str, recursive: bool) -> Result<(), CliError> {
    let (_device, storage) = open_storage(cli, false).await?;
    let path = RemotePath::parse(remote_path)?;
    let (parent, listed_path) = folder_parent(&storage, &path, cli.verbose).await?;
    let objects = if recursive {
        storage
            .list_objects_recursive(parent)
            .await
            .map_err(|e| CliError::from_mtp("list remote folder", e, cli.verbose))?
    } else {
        storage
            .list_objects(parent)
            .await
            .map_err(|e| CliError::from_mtp("list remote folder", e, cli.verbose))?
    };
    let rows: Vec<ObjectRow> = objects.iter().map(ObjectRow::from).collect();

    if cli.json {
        print_json(&LsRow {
            path: listed_path,
            recursive,
            objects: rows,
        })
    } else {
        for row in rows {
            let kind = if row.kind == "folder" { "DIR " } else { "FILE" };
            println!(
                "{} {:>12} handle={} {}",
                kind, row.size, row.handle, row.filename
            );
        }
        Ok(())
    }
}

async fn put(
    cli: &Cli,
    local_path: &Path,
    remote_path: &str,
    replace: bool,
    verify: bool,
) -> Result<(), CliError> {
    let metadata = tokio::fs::metadata(local_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("read local file: {e}")))?;
    if !metadata.is_file() {
        return Err(CliError::new(
            CliErrorKind::Other,
            "local path is not a regular file",
        ));
    }

    let local_filename = local_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::new(CliErrorKind::Other, "local file has no valid filename"))?;
    path::validate_component(local_filename)?;

    let (_device, storage) = open_storage(cli, true).await?;
    let remote_path = RemotePath::parse(remote_path)?;
    let target =
        path::resolve_upload_target(&storage, &remote_path, local_filename, cli.verbose).await?;

    let replaced = target.existing.is_some();
    if let Some(existing) = &target.existing {
        if !replace {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "remote file already exists; pass --replace to delete it first",
            ));
        }
        storage
            .delete(existing.handle)
            .await
            .map_err(|e| CliError::from_mtp("delete existing remote file", e, cli.verbose))?;
    }

    let file = tokio::fs::File::open(local_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("open local file: {e}")))?;
    let total_size = metadata.len();
    let stream = file_stream(file);
    let info = NewObjectInfo::file(target.filename.clone(), total_size);
    let mut last_percent = 101u64;
    let handle = storage
        .upload_with_progress(target.parent, info, stream, |progress| {
            print_progress(
                "upload",
                progress.bytes_transferred,
                total_size,
                &mut last_percent,
            );
            ControlFlow::Continue(())
        })
        .await
        .map_err(|e| CliError::from_mtp("upload file", e, cli.verbose))?;
    finish_progress();
    let mut verified = false;

    if verify {
        verify_remote_matches_local(&storage, handle, local_path, total_size, cli.verbose).await?;
        verified = true;
    }

    let row = PutRow {
        operation: "put",
        local_path: local_path.display().to_string(),
        remote_path: remote_path.raw().to_string(),
        filename: target.filename,
        handle: handle.0,
        bytes: total_size,
        replaced,
        verified,
    };

    if cli.json {
        print_json(&row)
    } else {
        println!(
            "uploaded {} ({} bytes) handle={}",
            row.filename, row.bytes, row.handle
        );
        if row.verified {
            println!("verified {}", row.filename);
        }
        Ok(())
    }
}

async fn get(
    cli: &Cli,
    remote_path: &str,
    local_path: &Path,
    replace: bool,
) -> Result<(), CliError> {
    let destination_exists = tokio::fs::try_exists(local_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("check local path: {e}")))?;
    if destination_exists && !replace {
        return Err(CliError::new(
            CliErrorKind::Other,
            "local file already exists; pass --replace to overwrite it",
        ));
    }

    let (_device, storage) = open_storage(cli, false).await?;
    let path = RemotePath::parse(remote_path)?;
    let object = match path::resolve_existing(&storage, &path, cli.verbose).await? {
        ExistingRemote::Root => {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "cannot download the storage root",
            ));
        }
        ExistingRemote::Object(object) if object.is_file() => object,
        ExistingRemote::Object(_) => {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "remote path is not a file",
            ));
        }
    };

    let mut download = storage
        .download_stream(object.handle)
        .await
        .map_err(|e| CliError::from_mtp("start download", e, cli.verbose))?;
    let temp_path = temp_download_path(local_path);
    let mut out = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("create local file: {e}")))?;
    let mut last_percent = 101u64;

    let download_result = async {
        while let Some(chunk) = download.next_chunk().await {
            let bytes = chunk.map_err(|e| CliError::from_mtp("download file", e, cli.verbose))?;
            out.write_all(&bytes).await.map_err(|e| {
                CliError::new(CliErrorKind::Transfer, format!("write local file: {e}"))
            })?;
            print_progress(
                "download",
                download.bytes_received(),
                download.size(),
                &mut last_percent,
            );
        }
        out.flush()
            .await
            .map_err(|e| CliError::new(CliErrorKind::Transfer, format!("flush local file: {e}")))?;
        Ok::<(), CliError>(())
    }
    .await;
    drop(out);

    if let Err(err) = download_result {
        let _ = tokio::fs::remove_file(&temp_path).await;
        finish_progress();
        return Err(err);
    }

    if destination_exists && replace {
        tokio::fs::remove_file(local_path)
            .await
            .map_err(|e| CliError::new(CliErrorKind::Other, format!("replace local file: {e}")))?;
    }
    tokio::fs::rename(&temp_path, local_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("replace local file: {e}")))?;
    finish_progress();

    let bytes = download.bytes_received();
    let handle = object.handle.0;
    let filename = object.filename;
    let row = GetRow {
        operation: "get",
        remote_path: path.raw().to_string(),
        local_path: local_path.display().to_string(),
        filename,
        handle,
        bytes,
    };

    if cli.json {
        print_json(&row)
    } else {
        println!("downloaded {} ({} bytes)", row.local_path, row.bytes);
        Ok(())
    }
}

async fn verify_remote_matches_local(
    storage: &Storage,
    handle: ObjectHandle,
    local_path: &Path,
    total_size: u64,
    verbose: bool,
) -> Result<(), CliError> {
    let mut remote = storage
        .download_stream(handle)
        .await
        .map_err(|e| CliError::from_mtp("verify download", e, verbose))?;
    let mut local = tokio::fs::File::open(local_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Verify, format!("verify local file: {e}")))?;
    let mut compared = 0u64;

    while let Some(chunk) = remote.next_chunk().await {
        let bytes = chunk.map_err(|e| CliError::from_mtp("verify download", e, verbose))?;
        let mut local_bytes = vec![0; bytes.len()];
        local
            .read_exact(&mut local_bytes)
            .await
            .map_err(|e| CliError::new(CliErrorKind::Verify, format!("verify local file: {e}")))?;
        if bytes.as_ref() != local_bytes.as_slice() {
            return Err(CliError::new(
                CliErrorKind::Verify,
                "verification failed: uploaded bytes differ from local file",
            ));
        }
        compared += bytes.len() as u64;
    }

    if compared != total_size {
        return Err(CliError::new(
            CliErrorKind::Verify,
            "verification failed: uploaded size differs from local file",
        ));
    }

    Ok(())
}

async fn mkdir(cli: &Cli, remote_path: &str) -> Result<(), CliError> {
    let (_device, storage) = open_storage(cli, true).await?;
    let path = RemotePath::parse(remote_path)?;
    if path.is_root() {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            "cannot create the storage root",
        ));
    }

    let components = path.components();
    let name = components
        .last()
        .expect("non-root path has final component");
    let parent_path = RemotePath::parse(&parent_path_string(components))?;
    let parent = match path::resolve_existing(&storage, &parent_path, cli.verbose).await? {
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
        .map_err(|e| CliError::from_mtp("list remote folder", e, cli.verbose))?;
    if siblings
        .iter()
        .any(|object| object.filename == name.as_str())
    {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            "remote object already exists",
        ));
    }

    let handle = storage
        .create_folder(parent, name)
        .await
        .map_err(|e| CliError::from_mtp("create remote folder", e, cli.verbose))?;
    let row = MkdirRow {
        operation: "mkdir",
        remote_path: remote_path.to_string(),
        filename: name.to_string(),
        handle: handle.0,
    };

    if cli.json {
        print_json(&row)
    } else {
        println!("created folder {} handle={}", row.remote_path, row.handle);
        Ok(())
    }
}

async fn rm(cli: &Cli, remote_path: &str, yes: bool) -> Result<(), CliError> {
    if !yes {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            "refusing to delete without --yes",
        ));
    }

    let (_device, storage) = open_storage(cli, true).await?;
    let path = RemotePath::parse(remote_path)?;
    let object = match path::resolve_existing(&storage, &path, cli.verbose).await? {
        ExistingRemote::Root => {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "refusing to delete the storage root",
            ));
        }
        ExistingRemote::Object(object) => object,
    };

    storage
        .delete(object.handle)
        .await
        .map_err(|e| CliError::from_mtp("delete remote object", e, cli.verbose))?;
    let kind = if object.is_folder() { "folder" } else { "file" };
    let row = RmRow {
        operation: "rm",
        remote_path: remote_path.to_string(),
        filename: object.filename,
        handle: object.handle.0,
        kind,
    };

    if cli.json {
        print_json(&row)
    } else {
        println!("deleted {} handle={}", row.remote_path, row.handle);
        Ok(())
    }
}

async fn rename(cli: &Cli, remote_path: &str, new_name: &str) -> Result<(), CliError> {
    path::validate_component(new_name)?;
    let (device, storage) = open_storage(cli, true).await?;
    ensure_rename_supported(&device)?;
    let path = RemotePath::parse(remote_path)?;
    let object = existing_object(&storage, &path, cli.verbose).await?;
    let old_name = object.filename.clone();
    ensure_rename_target_available(&storage, &path, object.handle, new_name, cli.verbose).await?;

    storage
        .rename(object.handle, new_name)
        .await
        .map_err(|e| CliError::from_mtp("rename remote object", e, cli.verbose))?;

    let row = RenameRow {
        operation: "rename",
        remote_path: remote_path.to_string(),
        old_name,
        new_name: new_name.to_string(),
        handle: object.handle.0,
        kind: if object.is_folder() { "folder" } else { "file" },
    };

    if cli.json {
        print_json(&row)
    } else {
        println!(
            "renamed {} -> {} handle={}",
            row.old_name, row.new_name, row.handle
        );
        Ok(())
    }
}

async fn move_remote(
    cli: &Cli,
    source_path: &str,
    destination_path: &str,
    replace: bool,
) -> Result<(), CliError> {
    let (device, storage) = open_storage(cli, true).await?;
    let source_path = RemotePath::parse(source_path)?;
    let destination_path = RemotePath::parse(destination_path)?;
    let source = existing_object(&storage, &source_path, cli.verbose).await?;
    let source_name = source.filename.clone();
    let source_kind = if source.is_folder() { "folder" } else { "file" };
    let target =
        resolve_object_destination(&storage, &destination_path, &source_name, cli.verbose).await?;
    let renamed = target.filename != source_name;

    if renamed {
        ensure_rename_supported(&device)?;
    }
    let replaced = target.existing.is_some();
    if let Some(existing) = &target.existing {
        ensure_not_same_object(source.handle, existing.handle)?;
        if !replace {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "destination already exists; pass --replace to delete it first",
            ));
        }
        storage
            .delete(existing.handle)
            .await
            .map_err(|e| CliError::from_mtp("delete existing destination", e, cli.verbose))?;
    }

    let parent = target.parent.unwrap_or(ObjectHandle::ROOT);
    storage
        .move_object(source.handle, parent, None)
        .await
        .map_err(|e| CliError::from_mtp("move remote object", e, cli.verbose))?;
    if renamed {
        storage
            .rename(source.handle, &target.filename)
            .await
            .map_err(|e| CliError::from_mtp("rename moved object", e, cli.verbose))?;
    }

    let row = MoveRow {
        operation: "mv",
        source_path: source_path.raw().to_string(),
        destination_path: destination_path.raw().to_string(),
        filename: target.filename,
        handle: source.handle.0,
        kind: source_kind,
        replaced,
        renamed,
    };

    if cli.json {
        print_json(&row)
    } else {
        println!(
            "moved {} -> {} handle={}",
            row.source_path, row.destination_path, row.handle
        );
        Ok(())
    }
}

async fn copy_remote(
    cli: &Cli,
    source_path: &str,
    destination_path: &str,
    replace: bool,
) -> Result<(), CliError> {
    let (device, storage) = open_storage(cli, true).await?;
    let source_path = RemotePath::parse(source_path)?;
    let destination_path = RemotePath::parse(destination_path)?;
    let source = existing_object(&storage, &source_path, cli.verbose).await?;
    let source_name = source.filename.clone();
    let source_kind = if source.is_folder() { "folder" } else { "file" };
    let target =
        resolve_object_destination(&storage, &destination_path, &source_name, cli.verbose).await?;
    let renamed = target.filename != source_name;

    if renamed {
        ensure_rename_supported(&device)?;
    }
    let replaced = target.existing.is_some();
    if let Some(existing) = &target.existing {
        ensure_not_same_object(source.handle, existing.handle)?;
        if !replace {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "destination already exists; pass --replace to delete it first",
            ));
        }
        storage
            .delete(existing.handle)
            .await
            .map_err(|e| CliError::from_mtp("delete existing destination", e, cli.verbose))?;
    }

    let parent = target.parent.unwrap_or(ObjectHandle::ROOT);
    let new_handle = storage
        .copy_object(source.handle, parent, None)
        .await
        .map_err(|e| CliError::from_mtp("copy remote object", e, cli.verbose))?;
    if renamed {
        storage
            .rename(new_handle, &target.filename)
            .await
            .map_err(|e| CliError::from_mtp("rename copied object", e, cli.verbose))?;
    }

    let row = CopyRow {
        operation: "cp",
        source_path: source_path.raw().to_string(),
        destination_path: destination_path.raw().to_string(),
        filename: target.filename,
        source_handle: source.handle.0,
        handle: new_handle.0,
        kind: source_kind,
        replaced,
        renamed,
    };

    if cli.json {
        print_json(&row)
    } else {
        println!(
            "copied {} -> {} handle={}",
            row.source_path, row.destination_path, row.handle
        );
        Ok(())
    }
}

async fn doctor(cli: &Cli) -> Result<(), CliError> {
    let devices = MtpDevice::list_devices_with_known(&cli.known)
        .map_err(|e| CliError::from_mtp("list devices", e, cli.verbose))?;
    if devices.is_empty() {
        if cli.json {
            print_json(&DoctorRow {
                devices: Vec::new(),
                opened: None,
                open_error: None,
                open_help: None,
                storages: Vec::new(),
            })?;
        } else {
            println!("devices: none");
        }
        return Err(CliError::new(CliErrorKind::NoDevice, "no MTP device found"));
    }
    let device_rows: Vec<DeviceRow> = devices.iter().map(DeviceRow::from).collect();
    if !cli.json {
        println!("devices: {} visible", devices.len());
        for device in &devices {
            println!("  {}", device.display());
        }
    }

    let device = match open_selected_device(cli).await {
        Ok(device) => device,
        Err(err) => {
            if cli.json {
                print_json(&DoctorRow {
                    devices: device_rows,
                    opened: None,
                    open_error: Some(err.to_string()),
                    open_help: err.help().map(str::to_string),
                    storages: Vec::new(),
                })?;
            }
            return Err(err);
        }
    };
    let opened = OpenedDeviceRow {
        manufacturer: device.device_info().manufacturer.clone(),
        model: device.device_info().model.clone(),
        serial_number: device.device_info().serial_number.clone(),
    };
    if !cli.json {
        println!("open: ok ({} {})", opened.manufacturer, opened.model);
    }

    let storages = device
        .storages()
        .await
        .map_err(|e| CliError::from_mtp("list storages", e, cli.verbose))?;
    let mut storage_rows = Vec::new();
    if !cli.json {
        println!("storages: {}", storages.len());
    }
    for (index, storage) in storages.iter().enumerate() {
        if !cli.json {
            println!(
                "  [{}] {} free={} access={:?}",
                index,
                storage.info().description,
                storage.info().free_space_bytes,
                storage.info().access_capability
            );
        }
        let root = storage
            .list_objects(None)
            .await
            .map_err(|e| CliError::from_mtp("list storage root", e, cli.verbose))?;
        let hints: Vec<String> = [
            "Download",
            "Downloads",
            "Documents",
            "Music",
            "Pictures",
            "Audiobooks",
            "Podcasts",
            "GARMIN",
        ]
        .into_iter()
        .filter(|name| {
            root.iter()
                .any(|object| object.is_folder() && object.filename == *name)
        })
        .map(str::to_string)
        .collect();
        if !cli.json {
            if hints.is_empty() {
                println!("      writable-folder hints: none found at root");
            } else {
                println!("      writable-folder hints: {}", hints.join(", "));
            }
        }
        storage_rows.push(DoctorStorageRow {
            storage: StorageRow::from_storage(index, storage),
            root_listed: true,
            writable_folder_hints: hints,
        });
    }

    if cli.json {
        print_json(&DoctorRow {
            devices: device_rows,
            opened: Some(opened),
            open_error: None,
            open_help: None,
            storages: storage_rows,
        })
    } else {
        Ok(())
    }
}

async fn open_storage(cli: &Cli, mutating: bool) -> Result<(MtpDevice, Storage), CliError> {
    let device = open_selected_device(cli).await?;
    let storages = device
        .storages()
        .await
        .map_err(|e| CliError::from_mtp("list storages", e, cli.verbose))?;
    if storages.is_empty() {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            "device has no storages",
        ));
    }

    let storage = if let Some(selection) = &cli.storage {
        select_storage(storages, selection)?
    } else if mutating && storages.len() > 1 {
        return Err(CliError::new(
            CliErrorKind::AmbiguousSelection,
            "multiple storages found; pass --storage INDEX_OR_ID for mutating commands",
        ));
    } else {
        storages
            .into_iter()
            .next()
            .expect("empty storages handled above")
    };

    Ok((device, storage))
}

async fn open_selected_device(cli: &Cli) -> Result<MtpDevice, CliError> {
    let builder = MtpDevice::builder()
        .known_devices(&cli.known)
        .timeout(Duration::from_secs(cli.timeout));

    if let Some(serial) = &cli.device {
        return builder
            .open_by_serial(serial)
            .await
            .map_err(|e| CliError::from_mtp("open device", e, cli.verbose));
    }

    if let Some(location) = cli.location {
        return builder
            .open_by_location(location)
            .await
            .map_err(|e| CliError::from_mtp("open device", e, cli.verbose));
    }

    let devices = MtpDevice::list_devices_with_known(&cli.known)
        .map_err(|e| CliError::from_mtp("list devices", e, cli.verbose))?;
    match devices.as_slice() {
        [] => Err(CliError::new(CliErrorKind::NoDevice, "no MTP device found")),
        [device] => MtpDevice::builder()
            .known_devices(&cli.known)
            .timeout(Duration::from_secs(cli.timeout))
            .open_by_location(device.location_id)
            .await
            .map_err(|e| CliError::from_mtp("open device", e, cli.verbose)),
        _ => Err(CliError::new(
            CliErrorKind::AmbiguousSelection,
            "multiple MTP devices found; pass --device SERIAL or --location LOCATION",
        )),
    }
}

fn select_storage(mut storages: Vec<Storage>, selection: &str) -> Result<Storage, CliError> {
    if let Ok(index) = selection.parse::<usize>() {
        if index < storages.len() {
            return Ok(storages.remove(index));
        }
    }

    let id = parse_storage_id(selection)?;
    let index = storages
        .iter()
        .position(|storage| storage.id() == StorageId(id))
        .ok_or_else(|| CliError::new(CliErrorKind::RemotePath, "storage not found"))?;
    Ok(storages.remove(index))
}

fn parse_storage_id(selection: &str) -> Result<u32, CliError> {
    if let Some(hex) = selection
        .strip_prefix("0x")
        .or_else(|| selection.strip_prefix("0X"))
    {
        return u32::from_str_radix(hex, 16)
            .map_err(|_| CliError::new(CliErrorKind::RemotePath, "invalid storage ID"));
    }

    if selection
        .chars()
        .any(|c| matches!(c, 'a'..='f' | 'A'..='F'))
    {
        return u32::from_str_radix(selection, 16)
            .map_err(|_| CliError::new(CliErrorKind::RemotePath, "invalid storage ID"));
    }

    selection
        .parse::<u32>()
        .map_err(|_| CliError::new(CliErrorKind::RemotePath, "invalid storage selection"))
}

async fn folder_parent(
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

fn parent_path_string(components: &[String]) -> String {
    if components.len() <= 1 {
        "/".to_string()
    } else {
        format!("/{}", components[..components.len() - 1].join("/"))
    }
}

async fn existing_object(
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

async fn resolve_object_destination(
    storage: &Storage,
    destination_path: &RemotePath,
    source_filename: &str,
    verbose: bool,
) -> Result<path::UploadTarget, CliError> {
    path::resolve_upload_target(storage, destination_path, source_filename, verbose).await
}

fn ensure_rename_supported(device: &MtpDevice) -> Result<(), CliError> {
    if device.supports_rename() {
        Ok(())
    } else {
        Err(CliError::new(
            CliErrorKind::Transfer,
            "device does not support rename; cannot change the destination filename",
        ))
    }
}

async fn ensure_rename_target_available(
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

fn ensure_not_same_object(source: ObjectHandle, destination: ObjectHandle) -> Result<(), CliError> {
    if source == destination {
        Err(CliError::new(
            CliErrorKind::RemotePath,
            "source and destination are the same remote object",
        ))
    } else {
        Ok(())
    }
}

fn temp_download_path(destination: &Path) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    parent.join(format!(".{name}.mtp-rs-{nonce}-{}.tmp", std::process::id()))
}

fn file_stream(
    file: tokio::fs::File,
) -> Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>> {
    Box::pin(futures::stream::unfold(file, |mut file| async move {
        let mut buf = vec![0; CHUNK_SIZE];
        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(Bytes::from(buf)), file))
            }
            Err(e) => Some((Err(e), file)),
        }
    }))
}

fn print_progress(label: &str, done: u64, total: u64, last_percent: &mut u64) {
    let percent = done.saturating_mul(100).checked_div(total).unwrap_or(100);
    if percent != *last_percent {
        eprint!("\r{}: {}% ({}/{})", label, percent, done, total);
        let _ = std::io::stderr().flush();
        *last_percent = percent;
    }
}

fn finish_progress() {
    eprintln!();
}

fn print_json<T: Serialize>(value: &T) -> Result<(), CliError> {
    serde_json::to_writer_pretty(std::io::stdout(), value)
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("write JSON: {e}")))?;
    println!();
    std::io::stdout()
        .flush()
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("flush JSON: {e}")))?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct DeviceRow {
    vendor_id: u16,
    product_id: u16,
    manufacturer: Option<String>,
    product: Option<String>,
    serial_number: Option<String>,
    location_id: u64,
    location: String,
    speed: Option<String>,
    match_reason: String,
}

impl From<&MtpDeviceInfo> for DeviceRow {
    fn from(info: &MtpDeviceInfo) -> Self {
        Self {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            manufacturer: info.manufacturer.clone(),
            product: info.product.clone(),
            serial_number: info.serial_number.clone(),
            location_id: info.location_id,
            location: format!("{:08x}", info.location_id),
            speed: info.speed.map(|speed| format!("{speed:?}")),
            match_reason: info.match_reason.as_str().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct InfoRow {
    manufacturer: String,
    model: String,
    serial_number: String,
    device_version: String,
    supports_rename: bool,
    storages: Vec<StorageRow>,
}

#[derive(Debug, Serialize)]
struct StorageRow {
    index: usize,
    id: String,
    id_raw: u32,
    description: String,
    volume_identifier: String,
    max_capacity: u64,
    free_space_bytes: u64,
    access_capability: String,
    storage_type: String,
    filesystem_type: String,
}

impl StorageRow {
    fn from_storage(index: usize, storage: &Storage) -> Self {
        Self {
            index,
            id: format!("{:08x}", storage.id().0),
            id_raw: storage.id().0,
            description: storage.info().description.clone(),
            volume_identifier: storage.info().volume_identifier.clone(),
            max_capacity: storage.info().max_capacity,
            free_space_bytes: storage.info().free_space_bytes,
            access_capability: format!("{:?}", storage.info().access_capability),
            storage_type: format!("{:?}", storage.info().storage_type),
            filesystem_type: format!("{:?}", storage.info().filesystem_type),
        }
    }
}

#[derive(Debug, Serialize)]
struct LsRow {
    path: String,
    recursive: bool,
    objects: Vec<ObjectRow>,
}

#[derive(Debug, Serialize)]
struct PutRow {
    operation: &'static str,
    local_path: String,
    remote_path: String,
    filename: String,
    handle: u32,
    bytes: u64,
    replaced: bool,
    verified: bool,
}

#[derive(Debug, Serialize)]
struct GetRow {
    operation: &'static str,
    remote_path: String,
    local_path: String,
    filename: String,
    handle: u32,
    bytes: u64,
}

#[derive(Debug, Serialize)]
struct MkdirRow {
    operation: &'static str,
    remote_path: String,
    filename: String,
    handle: u32,
}

#[derive(Debug, Serialize)]
struct RmRow {
    operation: &'static str,
    remote_path: String,
    filename: String,
    handle: u32,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct RenameRow {
    operation: &'static str,
    remote_path: String,
    old_name: String,
    new_name: String,
    handle: u32,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct MoveRow {
    operation: &'static str,
    source_path: String,
    destination_path: String,
    filename: String,
    handle: u32,
    kind: &'static str,
    replaced: bool,
    renamed: bool,
}

#[derive(Debug, Serialize)]
struct CopyRow {
    operation: &'static str,
    source_path: String,
    destination_path: String,
    filename: String,
    source_handle: u32,
    handle: u32,
    kind: &'static str,
    replaced: bool,
    renamed: bool,
}

#[derive(Debug, Serialize)]
struct DoctorRow {
    devices: Vec<DeviceRow>,
    opened: Option<OpenedDeviceRow>,
    open_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_help: Option<String>,
    storages: Vec<DoctorStorageRow>,
}

#[derive(Debug, Serialize)]
struct OpenedDeviceRow {
    manufacturer: String,
    model: String,
    serial_number: String,
}

#[derive(Debug, Serialize)]
struct DoctorStorageRow {
    storage: StorageRow,
    root_listed: bool,
    writable_folder_hints: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ObjectRow {
    handle: u32,
    storage_id: u32,
    parent: u32,
    filename: String,
    kind: String,
    size: u64,
    format: String,
}

impl From<&ObjectInfo> for ObjectRow {
    fn from(info: &ObjectInfo) -> Self {
        Self {
            handle: info.handle.0,
            storage_id: info.storage_id.0,
            parent: info.parent.0,
            filename: info.filename.clone(),
            kind: if info.is_folder() { "folder" } else { "file" }.to_string(),
            size: info.size,
            format: format!("{:?}", info.format),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parent_path_for_one_component_is_root() {
        assert_eq!(parent_path_string(&["Music".to_string()]), "/");
    }

    #[test]
    fn parent_path_for_nested_component_is_parent() {
        assert_eq!(
            parent_path_string(&["GARMIN".to_string(), "APPS".to_string()]),
            "/GARMIN"
        );
    }

    #[cfg(feature = "virtual-device")]
    struct VirtualCliFixture {
        _tempdir: tempfile::TempDir,
        serial: String,
        location_id: u64,
    }

    #[cfg(feature = "virtual-device")]
    impl Drop for VirtualCliFixture {
        fn drop(&mut self) {
            mtp_rs::unregister_virtual_device(self.location_id);
        }
    }

    #[cfg(feature = "virtual-device")]
    fn virtual_cli_fixture() -> VirtualCliFixture {
        let tempdir = tempfile::tempdir().unwrap();
        let backing_dir = tempdir.path().join("storage");
        std::fs::create_dir(&backing_dir).unwrap();
        let serial = format!(
            "cli-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let config = mtp_rs::VirtualDeviceConfig {
            manufacturer: "TestCorp".into(),
            model: "CLI Device".into(),
            serial: serial.clone(),
            storages: vec![mtp_rs::VirtualStorageConfig {
                description: "Internal Storage".into(),
                capacity: 64 * 1024 * 1024,
                backing_dir,
                read_only: false,
            }],
            supports_rename: true,
            event_poll_interval: Duration::ZERO,
            watch_backing_dirs: false,
        };
        let info = mtp_rs::register_virtual_device(&config);

        VirtualCliFixture {
            _tempdir: tempdir,
            serial,
            location_id: info.location_id,
        }
    }

    #[cfg(feature = "virtual-device")]
    fn test_cli(serial: &str, json: bool) -> Cli {
        Cli {
            device: Some(serial.to_string()),
            location: None,
            storage: None,
            known: Vec::new(),
            timeout: 30,
            json,
            verbose: false,
            command: Command::Doctor,
        }
    }

    #[cfg(feature = "virtual-device")]
    #[tokio::test]
    async fn virtual_device_cli_file_lifecycle() {
        let fixture = virtual_cli_fixture();
        let cli = test_cli(&fixture.serial, true);
        let local = fixture._tempdir.path().join("local.txt");
        let downloaded = fixture._tempdir.path().join("downloaded.txt");
        let copied = fixture._tempdir.path().join("copied.txt");
        let moved = fixture._tempdir.path().join("moved.txt");

        devices(&cli).unwrap();
        info(&cli).await.unwrap();
        ls(&cli, "/", false).await.unwrap();
        mkdir(&cli, "/Upload").await.unwrap();

        tokio::fs::write(&local, b"hello virtual cli")
            .await
            .unwrap();
        put(&cli, &local, "/Upload/remote.txt", false, true)
            .await
            .unwrap();
        ls(&cli, "/Upload", false).await.unwrap();
        get(&cli, "/Upload/remote.txt", &downloaded, false)
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&downloaded).await.unwrap(),
            "hello virtual cli"
        );

        tokio::fs::write(&local, b"updated virtual cli")
            .await
            .unwrap();
        put(&cli, &local, "/Upload/remote.txt", true, true)
            .await
            .unwrap();
        get(&cli, "/Upload/remote.txt", &downloaded, true)
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&downloaded).await.unwrap(),
            "updated virtual cli"
        );

        rename(&cli, "/Upload/remote.txt", "renamed.txt")
            .await
            .unwrap();
        mkdir(&cli, "/Archive").await.unwrap();
        copy_remote(&cli, "/Upload/renamed.txt", "/Archive/copied.txt", false)
            .await
            .unwrap();
        get(&cli, "/Archive/copied.txt", &copied, false)
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&copied).await.unwrap(),
            "updated virtual cli"
        );

        move_remote(&cli, "/Archive/copied.txt", "/Upload/moved.txt", false)
            .await
            .unwrap();
        get(&cli, "/Upload/moved.txt", &moved, false).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&moved).await.unwrap(),
            "updated virtual cli"
        );
        rm(&cli, "/Upload/moved.txt", true).await.unwrap();
        assert!(get(&cli, "/Upload/moved.txt", &moved, true).await.is_err());
    }
}
