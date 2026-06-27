//! Device and storage selection.
//!
//! Translates the global `--device` / `--location` / `--storage` / `--known`
//! flags into an opened `MtpDevice` and a chosen `Storage`. Subcommands call
//! `open_selected_device` (read-only metadata) or `open_storage` (anything
//! that touches a specific storage's contents).

use mtp_rs::mtp::Storage;
use mtp_rs::{MtpDevice, StorageId};
use std::time::Duration;

use super::args::Cli;
use super::error::{CliError, CliErrorKind};

pub async fn open_storage(cli: &Cli, mutating: bool) -> Result<(MtpDevice, Storage), CliError> {
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

pub async fn open_selected_device(cli: &Cli) -> Result<MtpDevice, CliError> {
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

fn parse_storage_id(selection: &str) -> Result<u64, CliError> {
    if let Some(hex) = selection
        .strip_prefix("0x")
        .or_else(|| selection.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16)
            .map_err(|_| CliError::new(CliErrorKind::RemotePath, "invalid storage ID"));
    }

    if selection
        .chars()
        .any(|c| matches!(c, 'a'..='f' | 'A'..='F'))
    {
        return u64::from_str_radix(selection, 16)
            .map_err(|_| CliError::new(CliErrorKind::RemotePath, "invalid storage ID"));
    }

    selection
        .parse::<u64>()
        .map_err(|_| CliError::new(CliErrorKind::RemotePath, "invalid storage selection"))
}
