use mtp_rs::{ByteRange, MtpDevice, Storage};
use serde::Serialize;
use std::time::Duration;

use crate::cli::args::{Cli, DoctorArgs};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    cancel_probe: Option<CancelProbeRow>,
}

#[derive(Debug, Serialize)]
struct OpenedDeviceRow {
    manufacturer: String,
    model: String,
    serial_number: String,
    capabilities: CapabilitiesRow,
}

#[derive(Debug, Serialize)]
struct CapabilitiesRow {
    can_upload: bool,
    can_delete: bool,
    can_rename: bool,
    can_move: bool,
    can_copy: bool,
    can_create_folder: bool,
    supports_partial_download: bool,
    supports_thumbnails: bool,
    supports_events: bool,
}

/// Outcome of the `--probe-cancel` cancel-health check (the #18 reproducer):
/// download a file, cancel mid-stream, and see whether the session survives.
#[derive(Debug, Serialize)]
struct CancelProbeRow {
    /// What the probe did, in one word: `healthy`, `wedged_recovered`,
    /// `errored`, or `skipped`.
    outcome: &'static str,
    /// Human-readable detail (file used, error text, or why it was skipped).
    detail: String,
}

#[derive(Debug, Serialize)]
struct DoctorStorageRow {
    storage: StorageRow,
    root_listed: bool,
    writable_folder_hints: Vec<String>,
}

pub async fn run(cli: &Cli, args: &DoctorArgs) -> Result<(), CliError> {
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
                cancel_probe: None,
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
                    cancel_probe: None,
                })?;
            }
            return Err(err);
        }
    };
    let caps = device.capabilities();
    let opened = OpenedDeviceRow {
        manufacturer: device.device_info().manufacturer.clone(),
        model: device.device_info().model.clone(),
        serial_number: device.device_info().serial_number.clone(),
        capabilities: CapabilitiesRow {
            can_upload: caps.can_upload,
            can_delete: caps.can_delete,
            can_rename: caps.can_rename,
            can_move: caps.can_move,
            can_copy: caps.can_copy,
            can_create_folder: caps.can_create_folder,
            supports_partial_download: caps.supports_partial_download,
            supports_thumbnails: caps.supports_thumbnails,
            supports_events: caps.supports_events,
        },
    };
    if !cli.json {
        println!("open: ok ({} {})", opened.manufacturer, opened.model);
        let c = &opened.capabilities;
        println!(
            "capabilities: upload={} delete={} rename={} move={} copy={} mkdir={} partial_download={} thumbnails={} events={}",
            c.can_upload,
            c.can_delete,
            c.can_rename,
            c.can_move,
            c.can_copy,
            c.can_create_folder,
            c.supports_partial_download,
            c.supports_thumbnails,
            c.supports_events,
        );
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
                "  [{}] {} free={} access={}",
                index,
                storage.info().description,
                storage.info().free_space,
                if storage.info().is_writable {
                    "ReadWrite"
                } else {
                    "ReadOnly"
                }
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

    let cancel_probe = if args.probe_cancel {
        let row = match storages.first() {
            Some(storage) => cancel_health_probe(storage).await,
            None => CancelProbeRow {
                outcome: "skipped",
                detail: "no storage to probe".to_string(),
            },
        };
        if !cli.json {
            println!("cancel-probe: {} ({})", row.outcome, row.detail);
        }
        Some(row)
    } else {
        None
    };

    if cli.json {
        return print_json(&DoctorRow {
            devices: device_rows,
            opened: Some(opened),
            open_error: None,
            open_help: None,
            storages: storage_rows,
            cancel_probe,
        });
    }

    Ok(())
}

/// The cancel-health probe (`--probe-cancel`): download the largest file at the
/// storage root, cancel mid-stream, and classify what happened. This is the #18
/// reproducer — a device that wedges on a large-backlog cancel returns
/// `DeviceReset` here, which the plain listing above can't reveal. Read-only.
async fn cancel_health_probe(storage: &Storage) -> CancelProbeRow {
    let root = match storage.list_objects(None).await {
        Ok(objects) => objects,
        Err(e) => {
            return CancelProbeRow {
                outcome: "skipped",
                detail: format!("could not list storage root: {e}"),
            };
        }
    };
    // Biggest file => biggest in-flight backlog => best chance to surface a
    // large-backlog wedge (#18).
    let Some(target) = root.iter().filter(|o| o.is_file()).max_by_key(|o| o.size) else {
        return CancelProbeRow {
            outcome: "skipped",
            detail: "no file at storage root to probe".to_string(),
        };
    };

    let mut download = match storage.download(target.handle, ByteRange::Full).await {
        Ok(download) => download,
        Err(e) => {
            return CancelProbeRow {
                outcome: "errored",
                detail: format!("could not start download of '{}': {e}", target.filename),
            };
        }
    };
    // Read one chunk so there is an in-flight transfer to cancel.
    let _ = download.next_chunk().await;

    match download.cancel(Duration::from_millis(300)).await {
        Ok(()) => CancelProbeRow {
            outcome: "healthy",
            detail: format!(
                "cancelled '{}' ({} bytes); session survived",
                target.filename, target.size
            ),
        },
        Err(mtp_rs::Error::DeviceReset) => CancelProbeRow {
            outcome: "wedged_recovered",
            detail: format!(
                "cancel wedged the device on '{}' ({} bytes); the library reset it to recover (#18). \
                 Reopen quietly to continue, and prefer download_windowed for interruptible reads",
                target.filename, target.size
            ),
        },
        Err(e) => CancelProbeRow {
            outcome: "errored",
            detail: format!("cancel returned an error: {e}"),
        },
    }
}
