use serde::Serialize;

use crate::cli::args::Cli;
use crate::cli::device::open_selected_device;
use crate::cli::error::CliError;
use crate::cli::output::{print_json, StorageRow};

#[derive(Debug, Serialize)]
struct InfoRow {
    manufacturer: String,
    model: String,
    serial_number: String,
    device_version: String,
    supports_rename: bool,
    supports_copy: bool,
    storages: Vec<StorageRow>,
}

pub async fn run(cli: &Cli) -> Result<(), CliError> {
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
        supports_copy: device.supports_copy(),
        storages: storages
            .iter()
            .enumerate()
            .map(|(index, storage)| StorageRow::from_storage(index, storage))
            .collect(),
    };

    if cli.json {
        return print_json(&row);
    }

    println!(
        "{} {} serial={} version={}",
        row.manufacturer, row.model, row.serial_number, row.device_version
    );
    println!("supports rename: {}", row.supports_rename);
    println!("supports copy: {}", row.supports_copy);
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
