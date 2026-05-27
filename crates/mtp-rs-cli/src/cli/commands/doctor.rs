use mtp_rs::MtpDevice;
use serde::Serialize;

use crate::cli::args::Cli;
use crate::cli::device::open_selected_device;
use crate::cli::error::{CliError, CliErrorKind};
use crate::cli::output::{print_json, DeviceRow, StorageRow};

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

pub async fn run(cli: &Cli) -> Result<(), CliError> {
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
        return print_json(&DoctorRow {
            devices: device_rows,
            opened: Some(opened),
            open_error: None,
            open_help: None,
            storages: storage_rows,
        });
    }

    Ok(())
}
