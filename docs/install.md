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

## iPad (and Android): not available yet

There is no installable iPad or Android app. The iPad app needs a Mac with Xcode to build and sign; the steps and the helper
script (`scripts/ios.sh`) are in [phases/ipad.md](phases/ipad.md). Android is [phase 8](phases/phase-8.md).

## Not available yet

An AppImage, an apt repository, in-app updates and release builds in CI (the project has no remote).

## Running the installed app and a development build together

The app keeps its settings in `~/.config/notatnik/` and its data in `~/.local/share/notatnik/` and has no
single-instance lock, so a second copy shares both. `scripts/dev.sh` starts `cargo tauri dev` with `XDG_CONFIG_HOME`,
`XDG_DATA_HOME` and `XDG_CACHE_HOME` under `~/.dev-notatnik` (or `$NOTATNIK_DEV_HOME`), so the development build has its
own settings, graph history and webview storage. Delete that folder to start clean. The system keyring is not
separated: use a scratch graph in the development build, never the graph the installed app has open (two instances
writing one graph, and two sync loops on one server folder, are not supported).

## With make

The `Makefile` gives the commands short names; `make help` lists them. `make deb` builds the package, `make install` installs the newest `.deb` of the current version (building it first when there is none; it asks for sudo), `make uninstall` removes it, `make release RELEASE=X.Y.Z` is `scripts/release.sh X.Y.Z`, `make check` and `make dev` run the check script and the dev app.
