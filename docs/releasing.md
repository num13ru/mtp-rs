# Releasing to crates.io

This repo is a Cargo workspace with two published crates:

- `mtp-rs` (library, `crates/mtp-rs/`)
- `mtp-rs-cli` (CLI binary, `crates/mtp-rs-cli/`)

Publishing is manual. There's no CI automation for it.

## Versioning

- The library version drives breaking changes for downstream lib consumers and the CLI's `mtp-rs = "X.Y.Z"` dep. Bump it with normal SemVer rules.
- The CLI version can move independently. When the CLI ships a new feature or fix without a lib bump, only bump the CLI.
- When you bump the lib, also bump the CLI's `mtp-rs = "X.Y.Z"` dep to the new lib version (in `crates/mtp-rs-cli/Cargo.toml`). Usually you'll release both together.

## Steps

1. **Bump versions** in the relevant `Cargo.toml` files:
   - Library: `crates/mtp-rs/Cargo.toml` → `version = "X.Y.Z"`
   - CLI: `crates/mtp-rs-cli/Cargo.toml` → `version = "A.B.C"` and `mtp-rs = { version = "X.Y.Z", ... }` if the lib also moved
2. **Update `CHANGELOG.md`** at the repo root with one entry covering both crates. Tag each bullet with `[lib]`, `[cli]`, or `[workspace]` so readers can skim.
3. **Refresh `Cargo.lock`**:
   ```bash
   cargo update -p mtp-rs --precise X.Y.Z
   cargo update -p mtp-rs-cli --precise A.B.C
   ```
4. **Run `just check-all`** (format, clippy, test, doc, MSRV, audit, deny). The release commit must produce zero warnings, zero formatting diffs, zero doc-link issues. Re-run until clean.
5. **Dry run** to catch packaging issues:
   ```bash
   just release-dry
   ```
   This fully dry-runs the lib and prints the CLI's would-be file list.
   It does NOT dry-run the CLI publish because the CLI depends on the
   lib via `version = "X.Y.Z", path = "../mtp-rs"`. `cargo publish
   --dry-run -p mtp-rs-cli` resolves that version requirement against
   crates.io and rejects it while the new lib version is still local.
   The CLI gets its full dry-run in step 7 below, after the lib is on
   crates.io.
6. **Commit and tag**. Tag the workspace release with the lib version (since that's the API contract downstream consumers track):
   ```bash
   git commit -m "Prepare vX.Y.Z for release"
   git tag vX.Y.Z
   ```
   If you're shipping a CLI-only patch with no lib change, use `mtp-rs-cli-vA.B.C` instead.
7. **Publish in order**. Library first, CLI second (CLI depends on the published lib version):
   ```bash
   cargo publish -p mtp-rs
   # Wait ~30 seconds for crates.io index to update.
   # Now run the CLI dry-run (it can finally resolve the lib version):
   just release-dry-cli
   # Then publish:
   cargo publish -p mtp-rs-cli
   ```
8. **Push** the commit and tag:
   ```bash
   git push && git push --tags
   ```

## Prerequisites

- A crates.io API token configured via `cargo login`
- Both crates exclude non-shipping files via their own `exclude` lists in `Cargo.toml`. The lib also excludes `proptest-regressions/` to keep the package small.

## CLI-only patch flow

If only the CLI changes (no lib changes), you can release `mtp-rs-cli` without bumping the lib:

1. Bump `crates/mtp-rs-cli/Cargo.toml` version only.
2. Run `cargo update -p mtp-rs-cli --precise A.B.C`.
3. `just check-all`, then `cargo publish --dry-run -p mtp-rs-cli`.
4. Tag as `mtp-rs-cli-vA.B.C`.
5. Publish: `cargo publish -p mtp-rs-cli`.

## Previous releases

See [CHANGELOG.md](../CHANGELOG.md) for the full history. Git tags (`v0.1.0`, `v0.2.0`, etc.) mark each library release commit. CLI-only releases use the `mtp-rs-cli-v*` prefix.
