# Install and versions (Linux)

## Making a release

`scripts/release.sh <X.Y.Z>` sets the version, runs `scripts/check.sh`, builds the `.deb` (`cargo tauri build --bundles deb`),
copies it with a SHA-256 file into `dist/` (ignored by git), commits the version change and tags it `vX.Y.Z`. It refuses a
dirty tree, a branch other than `main`, an existing tag and a version that is not higher than the current one, and it puts
the version files back if a step fails. Nothing is pushed.

- `scripts/release.sh` alone prints the current version and the latest tags.
- `--skip-check` skips `scripts/check.sh` (use it only right after a green check); `--dry-run` validates and stops.
- The version lives in `Cargo.toml` (`[workspace.package]`, used by every crate) and `src-tauri/tauri.conf.json`; the script
  keeps them equal. Building needs the Tauri system libraries (WebKitGTK) and the Node setup of `CLAUDE.md`.

## Installing

```bash
sudo apt install ./dist/Notatnik_<version>_amd64.deb     # install or upgrade; apt adds the dependencies
sudo apt install --allow-downgrades ./Notatnik_<older>_amd64.deb   # go back to an older package
sudo apt remove notatnik
sha256sum -c dist/Notatnik_<version>_amd64.deb.sha256    # check a copied package
```

Graphs and settings live in the graph folders and the user's config directory, not in the package, so installing,
upgrading and removing never touch notes. Keep the old `.deb` files if you want to go back: nothing archives them. Take a
backup (Settings → Backup) before a big upgrade.

## Not available yet

An AppImage, an apt repository, in-app updates and release builds in CI (the project has no remote).
