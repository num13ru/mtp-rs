use serde::Serialize;

use crate::cli::args::{Cli, RenameArgs};
use crate::cli::device::open_storage;
use crate::cli::error::CliError;
use crate::cli::helpers::{
    ensure_rename_supported, ensure_rename_target_available, existing_object,
};
use crate::cli::output::print_json;
use crate::cli::path::{self, RemotePath};

#[derive(Debug, Serialize)]
struct RenameRow {
    operation: &'static str,
    remote_path: String,
    old_name: String,
    new_name: String,
    handle: u32,
    kind: &'static str,
}

pub async fn run(cli: &Cli, args: &RenameArgs) -> Result<(), CliError> {
    path::validate_component(&args.new_name)?;
    let (device, storage) = open_storage(cli, true).await?;
    ensure_rename_supported(&device)?;
    let path = RemotePath::parse(&args.remote_path)?;
    let object = existing_object(&storage, &path, cli.verbose).await?;
    let old_name = object.filename.clone();
    ensure_rename_target_available(&storage, &path, object.handle, &args.new_name, cli.verbose)
        .await?;

    storage
        .rename(object.handle, &args.new_name)
        .await
        .map_err(|e| CliError::from_mtp("rename remote object", e, cli.verbose))?;

    let row = RenameRow {
        operation: "rename",
        remote_path: args.remote_path.clone(),
        old_name,
        new_name: args.new_name.clone(),
        handle: object.handle.0,
        kind: if object.is_folder() { "folder" } else { "file" },
    };

    if cli.json {
        return print_json(&row);
    }

    println!(
        "renamed {} -> {} handle={}",
        row.old_name, row.new_name, row.handle
    );
    Ok(())
}
