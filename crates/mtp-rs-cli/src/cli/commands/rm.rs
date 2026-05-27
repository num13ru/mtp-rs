use serde::Serialize;

use crate::cli::args::{Cli, RmArgs};
use crate::cli::device::open_storage;
use crate::cli::error::{CliError, CliErrorKind};
use crate::cli::output::print_json;
use crate::cli::path::{self, ExistingRemote, RemotePath};

#[derive(Debug, Serialize)]
struct RmRow {
    operation: &'static str,
    remote_path: String,
    filename: String,
    handle: u32,
    kind: &'static str,
}

pub async fn run(cli: &Cli, args: &RmArgs) -> Result<(), CliError> {
    if !args.yes {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            "refusing to delete without --yes",
        ));
    }

    let (_device, storage) = open_storage(cli, true).await?;
    let path = RemotePath::parse(&args.remote_path)?;
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
        remote_path: args.remote_path.clone(),
        filename: object.filename,
        handle: object.handle.0,
        kind,
    };

    if cli.json {
        return print_json(&row);
    }

    println!("deleted {} handle={}", row.remote_path, row.handle);
    Ok(())
}
