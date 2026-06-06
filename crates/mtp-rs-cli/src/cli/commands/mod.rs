//! Subcommand implementations.
//!
//! Each subcommand exposes a single `pub async fn run(...)` (or `pub fn run`
//! for sync ones) that takes the parsed `Cli` plus any command-specific args.
//! Per-command output structs stay private to their file; shared output
//! shapes live in `super::output`.

pub mod cp;
pub mod devices;
pub mod doctor;
pub mod get;
pub mod info;
pub mod ls;
pub mod mkdir;
pub mod mv;
pub mod put;
pub mod rename;
pub mod reset;
pub mod rm;
