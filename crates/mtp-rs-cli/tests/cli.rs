#![cfg(feature = "virtual-device")]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SERIAL: &str = "cli-process-test";

struct CliFixture {
    _tempdir: tempfile::TempDir,
    backing_dir: PathBuf,
}

impl CliFixture {
    fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let backing_dir = tempdir.path().join("storage");
        std::fs::create_dir(&backing_dir).unwrap();
        Self {
            _tempdir: tempdir,
            backing_dir,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mtp-rs"));
        command
            .env("__MTP_RS_TEST_VIRTUAL_ROOT", &self.backing_dir)
            .env("__MTP_RS_TEST_VIRTUAL_SERIAL", SERIAL);
        command
    }

    fn run_json(&self, args: &[&str]) -> Value {
        let output = self.output(args);
        assert!(
            output.status.success(),
            "command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
            panic!(
                "stdout is not valid JSON: {err}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }

    fn output(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
}

#[test]
fn devices_json_lists_virtual_device() {
    let fixture = CliFixture::new();

    let value = fixture.run_json(&["--json", "devices"]);
    let devices = value.as_array().expect("devices output is an array");
    let device = devices
        .iter()
        .find(|device| device["serial_number"] == SERIAL)
        .expect("virtual device is listed");

    assert_eq!(device["manufacturer"], "TestCorp");
    assert_eq!(device["product"], "CLI Test Device");
    assert_eq!(device["match_reason"], "known_vid_pid");
}

#[test]
fn file_lifecycle_through_cli_binary_emits_json() {
    let fixture = CliFixture::new();
    let local = fixture._tempdir.path().join("local.txt");
    let downloaded = fixture._tempdir.path().join("downloaded.txt");
    std::fs::write(&local, b"hello from cli process").unwrap();

    let info = fixture.run_json(&["--json", "--device", SERIAL, "info"]);
    assert_eq!(info["serial_number"], SERIAL);
    assert_eq!(info["storages"][0]["description"], "Internal Storage");

    let mkdir = fixture.run_json(&["--json", "--device", SERIAL, "mkdir", "/Upload"]);
    assert_eq!(mkdir["operation"], "mkdir");
    assert_eq!(mkdir["remote_path"], "/Upload");

    let put = fixture.run_json(&[
        "--json",
        "--device",
        SERIAL,
        "put",
        path_str(&local),
        "/Upload/remote.txt",
        "--verify",
    ]);
    assert_eq!(put["operation"], "put");
    assert_eq!(put["remote_path"], "/Upload/remote.txt");
    assert_eq!(put["verified"], true);

    let listing = fixture.run_json(&["--json", "--device", SERIAL, "ls", "/Upload"]);
    let objects = listing["objects"].as_array().expect("objects is an array");
    assert!(objects
        .iter()
        .any(|object| object["filename"] == "remote.txt"));

    let get = fixture.run_json(&[
        "--json",
        "--device",
        SERIAL,
        "get",
        "/Upload/remote.txt",
        path_str(&downloaded),
    ]);
    assert_eq!(get["operation"], "get");
    assert_eq!(
        std::fs::read_to_string(&downloaded).unwrap(),
        "hello from cli process"
    );

    let rm = fixture.run_json(&[
        "--json",
        "--device",
        SERIAL,
        "rm",
        "/Upload/remote.txt",
        "--yes",
    ]);
    assert_eq!(rm["operation"], "rm");
    assert_eq!(rm["remote_path"], "/Upload/remote.txt");
}

#[test]
fn parser_errors_cross_process_boundary() {
    let fixture = CliFixture::new();

    let output = fixture.output(&["--device", "one", "--location", "0x1", "info"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot be used with")
            || String::from_utf8_lossy(&output.stderr).contains("unexpected argument")
    );

    let output = fixture.output(&["--known", "not-a-vid-pid", "devices"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected VID:PID"));
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("test path is valid UTF-8")
}
