use serde::Serialize;

use crate::cli::args::{Cli, RemotePathArg};
use crate::cli::device::open_storage;
use crate::cli::error::{CliError, CliErrorKind};
use crate::cli::helpers::parent_path_string;
use crate::cli::output::print_json;
use crate::cli::path::{self, ExistingRemote, RemotePath};

#[derive(Debug, Serialize)]
struct MkdirRow {
    operation: &'static str,
    remote_path: String,
    filename: String,
    handle: u32,
}

pub async fn run(cli: &Cli, args: &RemotePathArg) -> Result<(), CliError> {
    let (_device, storage) = open_storage(cli, true).await?;
    let path = RemotePath::parse(&args.remote_path)?;
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
        remote_path: args.remote_path.clone(),
        filename: name.to_string(),
        handle: handle.0,
    };

    if cli.json {
        return print_json(&row);
    }

    println!("created folder {} handle={}", row.remote_path, row.handle);
    Ok(())
}
