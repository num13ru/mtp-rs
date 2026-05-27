use mtp_rs::MtpDevice;

use crate::cli::args::Cli;
use crate::cli::error::CliError;
use crate::cli::output::{print_json, DeviceRow};

pub fn run(cli: &Cli) -> Result<(), CliError> {
    let devices = MtpDevice::list_devices_with_known(&cli.known)
        .map_err(|e| CliError::from_mtp("list devices", e, cli.verbose))?;
    let rows: Vec<DeviceRow> = devices.iter().map(DeviceRow::from).collect();

    if cli.json {
        return print_json(&rows);
    }

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
