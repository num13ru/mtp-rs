use serde::Serialize;

use crate::cli::args::{Cli, LsArgs};
use crate::cli::device::open_storage;
use crate::cli::error::CliError;
use crate::cli::helpers::folder_parent;
use crate::cli::output::{print_json, ObjectRow};
use crate::cli::path::RemotePath;

#[derive(Debug, Serialize)]
struct LsRow {
    path: String,
    recursive: bool,
    objects: Vec<ObjectRow>,
}

pub async fn run(cli: &Cli, args: &LsArgs) -> Result<(), CliError> {
    let (_device, storage) = open_storage(cli, false).await?;
    let path = RemotePath::parse(&args.remote_path)?;
    let (parent, listed_path) = folder_parent(&storage, &path, cli.verbose).await?;
    let objects = if args.recursive {
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
        return print_json(&LsRow {
            path: listed_path,
            recursive: args.recursive,
            objects: rows,
        });
    }

    for row in rows {
        let kind = if row.kind == "folder" { "DIR " } else { "FILE" };
        println!(
            "{} {:>12} handle={} {}",
            kind, row.size, row.handle, row.filename
        );
    }
    Ok(())
}
