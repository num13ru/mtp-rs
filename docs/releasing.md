# Releasing to crates.io

Publishing is manual — there's no CI automation for it.

## Steps

1. **Bump version** in `Cargo.toml`
2. **Update `CHANGELOG.md`** — set the new version and date
3. **Run all checks**: `just check-all` (includes MSRV, audit, license)
4. **Commit and tag**:
   ```bash
   git commit -m "Prepare vX.Y.Z for release"
   git tag vX.Y.Z
   ```
5. **Dry run** to catch packaging issues:
   ```bash
   cargo publish --dry-run
   ```
6. **Publish**:
   ```bash
   cargo publish
   ```
7. **Push** the commit and tag:
   ```bash
   git push && git push --tags
   ```

## Prerequisites

- A crates.io API token configured via `cargo login`
- The `exclude` list in `Cargo.toml` keeps the published package small (strips `.github/`, `docs/`, `justfile`, etc.)

## Previous releases

- **v0.1.0** — commit `96b6088`, tag `v0.1.0`
