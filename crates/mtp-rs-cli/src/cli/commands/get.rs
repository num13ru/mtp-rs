use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;

use crate::cli::args::{Cli, GetArgs};
use crate::cli::device::open_storage;
use crate::cli::error::{CliError, CliErrorKind};
use crate::cli::output::{finish_progress, print_json, print_progress};
use crate::cli::path::{self, ExistingRemote, RemotePath};

#[derive(Debug, Serialize)]
struct GetRow {
    operation: &'static str,
    remote_path: String,
    local_path: String,
    filename: String,
    handle: u32,
    bytes: u64,
}

pub async fn run(cli: &Cli, args: &GetArgs) -> Result<(), CliError> {
    let destination_exists = tokio::fs::try_exists(&args.local_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("check local path: {e}")))?;
    if destination_exists && !args.replace {
        return Err(CliError::new(
            CliErrorKind::Other,
            "local file already exists; pass --replace to overwrite it",
        ));
    }

    let (_device, storage) = open_storage(cli, false).await?;
    let path = RemotePath::parse(&args.remote_path)?;
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
    let temp_path = temp_download_path(&args.local_path);
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

    if destination_exists && args.replace {
        tokio::fs::remove_file(&args.local_path)
            .await
            .map_err(|e| CliError::new(CliErrorKind::Other, format!("replace local file: {e}")))?;
    }
    tokio::fs::rename(&temp_path, &args.local_path)
        .await
        .map_err(|e| CliError::new(CliErrorKind::Other, format!("replace local file: {e}")))?;
    finish_progress();

    let row = GetRow {
        operation: "get",
        remote_path: path.raw().to_string(),
        local_path: args.local_path.display().to_string(),
        filename: object.filename,
        handle: object.handle.0,
        bytes: download.bytes_received(),
    };

    if cli.json {
        return print_json(&row);
    }

    println!("downloaded {} ({} bytes)", row.local_path, row.bytes);
    Ok(())
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
