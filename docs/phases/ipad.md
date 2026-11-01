# iPad app (I1 to I4)

**Goal:** the app on an iPad (and, later, an iPhone): the graph in the app's own storage, synced with a Nextcloud/WebDAV
server by the built-in sync, touch and Apple Pencil for writing and drawing.

**Requirements covered:** PL-5 (see `requirements.md`); drawing with the Pencil is DR-2/DR-8.
**Depends on:** WebDAV sync (SY-1..SY-8), drawings (DR-1..DR-11).
**Status:** the groundwork is built and tested on Linux (I1, the touch part of I2). **The iOS app itself has not been
built or run**: that needs a Mac with Xcode, an Apple account and an iPad, none of which this repository's development
machine has. I3 and I4 are written down, not done.

## Design

- **One shell, three platforms.** `src-tauri` is a library (`notatnik_desktop_lib`, `run()` with
  `#[cfg_attr(mobile, tauri::mobile_entry_point)]`) plus a three-line `main.rs` for the desktop. The same Rust core and
  the same Svelte UI run on iOS in a `WKWebView`; WebKit is also the engine of the Linux app, so what passes the WebKit
  browser tests (`scripts/check-webkit.sh`) is the closest check available without an iPad.
- **Storage.** An iOS app can write only inside its own container. The device folders (settings, caches, the backup
  vault) are `Dirs::under(<app data dir>)` instead of the XDG folders, and the graph is `<app data dir>/notes`, made on
  the first start and opened again afterwards: no start screen, no folder chooser. Getting the notes in and out is the
  built-in sync (connect the graph to a Nextcloud folder in Settings → Sync) and the backup snapshots.
- **Secrets.** Passwords of server logins go in the iOS Keychain (the `keyring` crate with `apple-native`), like the
  Secret Service on Linux. The backup vault keys are files in the app's container (protected by iOS file protection).
- **What a desktop has and a tablet lacks**, and what replaces it: hover (the expand arrow is always shown when the
  device has no hover), the right button (press and hold a bullet for the block menu), `Ctrl+V` of a picture (the
  webview's own `paste` event; the desktop-only `clipboard_image` command is not built), the folder chooser
  (not used on mobile). Keyboard shortcuts work with a hardware keyboard.
- **The Pencil** reports itself as a `pen` through Pointer Events: it draws, a finger moves and zooms the canvas
  (DR-2). The drawing editor needs no change.

## Tasks

### I1 The shell builds for mobile (done on Linux, not run on iOS)

- Library plus `main.rs`; `arboard`, `png` and `base64` only on a desktop; `keyring` with the Secret Service on Linux
  and the Keychain on macOS and iOS; device folders in the app data dir on mobile; the first graph; `scripts/ios.sh`;
  `bundle.iOS.minimumSystemVersion = 15.0`.
- **Verify:**
  - [x] the desktop app still builds and passes everything (`scripts/check.sh`)
  - [ ] `cargo check --target aarch64-apple-ios -p notatnik-desktop` passes on a Mac (the `cfg(mobile)` code is not
        compiled on Linux, so it is unchecked)

### I2 Touch (done, tested in an emulated tablet)

- `viewport-fit=cover` and safe-area padding (rounded corners, home indicator); the expand arrow is always visible when
  `(hover: none)` and 28 px wide for a finger; sidebar entries get more room with `(pointer: coarse)`; press and hold
  on a bullet opens the block menu and the release does not zoom into the block.
- **Verify:**
  - [x] in a touch-emulated 1024×768 browser the arrow's computed opacity is 1 without hover and it is at least 24 px wide
  - [x] press and hold on a bullet opens the menu and does not zoom; a short tap still zooms (Chromium and WebKit)
  - [ ] on a real iPad: the content is not under the rounded corners or the home indicator; scrolling a long page
        does not trigger the block menu; dragging a bullet to move a block works with a finger (HTML5 drag and drop
        on iOS is untested)

### I3 First build on a Mac (not done)

1. A Mac with macOS 13 or newer, **Xcode** from the App Store (open it once, accept the licence, install the iOS
   platform under Settings → Platforms), **Rust** (`rustup`), **Node** with `pnpm`, and an **Apple ID** (a free one lets
   you run the app on your own iPad for 7 days at a time; the paid Developer Program, 99 USD a year, gives
   TestFlight and a year).
2. Find your **team ID** (developer.apple.com/account → Membership, or Xcode → Settings → Accounts) and
   `export APPLE_DEVELOPMENT_TEAM=XXXXXXXXXX`.
3. `scripts/ios.sh init` makes the Xcode project in `src-tauri/gen/apple` (commit it once it works).
4. `scripts/ios.sh dev` runs the app on the iOS simulator; with an iPad connected by cable (and Developer Mode on:
   Settings → Privacy & Security → Developer Mode) it runs on the iPad. The first time, trust the developer on the
   iPad (Settings → General → VPN & Device Management).
5. `scripts/ios.sh build` makes an `.ipa`; with a paid account, upload it to TestFlight from Xcode (Product → Archive →
   Distribute App) and install it from the TestFlight app.
- **Verify:**
  - [ ] the app starts on the simulator and creates `notes` with today's journal; editing works
  - [ ] it runs on a real iPad
  - [ ] Settings → Sync connects the graph to a Nextcloud folder and the notes come down; an edit on the iPad reaches
        the desktop
  - [ ] the Keychain keeps the server password across a restart
  - [ ] a drawing made with the Apple Pencil is saved and opens on the desktop; the Pencil does not scroll the page
        while drawing; a finger moves and zooms the canvas

### I4 What is likely to need work after the first run (not done)

- Pasting and dropping pictures; the file chooser for `/file` (the dialog plugin on iOS); opening links and files
  (`open_external`, `open_asset` use the opener plugin, which behaves differently on iOS).
- Background sync: iOS suspends the app, so the built-in sync runs while the app is open, on start and resume.
- A layout for portrait and split-view widths; an iPhone layout (the sidebar as a drawer).
- App icon sizes and a launch screen (`cargo tauri icon` makes the set from one image).
