# Android: open and sync notes through WebDAV (AS1 to AS5)

**Goal:** on an Android phone or tablet, connect to a Nextcloud/WebDAV server, download an existing graph, edit it, and
have the changes reach the server (and the desktop) the way they do on the desktop. Same engine, same form, same
conflict handling.

**Requirements covered:** SY-1..SY-9 (unchanged, now also on Android), SY-10 (new), EN-10 (key in the keystore, opt-in).
**Depends on:** WebDAV sync (`docs/webdav.md`), the Android shell (`docs/phases/phase-8.md` P8.1/P8.2).
**Status:** AS1 built and verified on the x86_64 emulator, debug and release build (2026-11-02), apart from the check
script; the route of AS2 and AS3 and half of AS4 were walked through on the emulator against the Docker Nextcloud. **Not
tried: a real phone or tablet, an `https` server, the arm64 build, airplane mode, a drawing, a conflict.** This replaces
P8.3 to P8.7 of `phase-8.md`: the sync engine and the WebDAV client already exist (`crates/sync`), so what is left is
only what Android lacks.
(`crates/sync`), so what is left is only what Android lacks.

## What is already there, and what is not

The engine, the form (`WebdavForm.svelte`), the replica folder and the conflict handling are shared code. The replica is
`<app data dir>/replicas/<name>/` because `device_dirs` puts the device folders below the app's data folder on mobile.
No Android permission is missing (`INTERNET` is in the manifest). The gaps:

1. **No place for the password (SY-7).** `KeyringSecrets` is built only for Linux, macOS and iOS. On Android the app
   uses `MemorySecrets`: the login would be lost at every restart, and a sync could never run unattended. Writing it
   into a file would break SY-7, so the answer is the Android Keystore.
2. **The start screen is reached with `Ctrl+K`.** A touch device has no such key. The route exists in Settings
   (_Switch graph…_) but has not been tried by touch, in a phone-sized window, with the keyboard up.
3. **Never run on Android:** the TLS roots (`ureq` with its bundled roots), the file watcher, sync after the app has
   been in the background.

## Design

- **Secrets: a small Tauri mobile plugin in Kotlin** (in `src-tauri`, not a third-party crate). It makes one AES-256-GCM
  key in the **Android Keystore** (hardware-backed where the device has it, not exportable), and keeps the encrypted
  values in a file in the app's private folder. Rust gets a `KeystoreSecrets` that implements the existing `Secrets`
  trait and calls the plugin through `run_mobile_plugin`. The trait does not change, so the sync, CalDAV and file
  sources code is untouched. A value that cannot be decrypted (a restored backup on another phone, a wiped keystore)
  reads as "no password", and the existing path asks for it again.
- **No folder chooser.** The graph from the server needs none: the replica folder is chosen by the app.
  The `notes` graph that is made on the first start stays a local graph; connecting it in place (SY-9) needs an empty
  server folder, so for an existing graph the route is _New graph on a server → Download and open_.
- **Plain `http://`** keeps the rule of SY-2 (this computer and the local network). The Rust HTTP client is not bound by
  Android's cleartext setting for web views, but the same address check applies. The emulator reaches the host as
  `10.0.2.2`, which the rule accepts, so a Docker Nextcloud (`dev/nextcloud/setup.sh`) can be used for testing.
- **Sync runs while the app is open, and once on every resume.** Android may freeze the process in the background, so
  nothing is promised there (background sync stays a stretch goal, as in `phase-8.md`).
- **A server with its own CA** (a private certificate) is not trusted by the bundled roots. Not handled here; the
  connection test says "certificate not trusted" and `docs/webdav.md` names it as a limit.

## Tasks

### AS1 Passwords in the Android Keystore

- [x] Kotlin plugin `secrets` (`get`, `set`, `delete`), AES-256-GCM key in the Keystore, values in a private file (`SecretsPlugin.kt`; R8 keeps it through `proguard-rules.pro`)
- [x] `KeystoreSecrets` in `src-tauri` for `cfg(target_os = "android")`, wired in with `with_secrets` (`src/keystore.rs`; the call runs on a thread of its own with a 10 s limit)
- [x] Settings texts that say "system's keyring" say something true on Android ("kept in the device's secure storage")

**Verify:**

- [x] On the emulator: connect to a server, force-stop the app, start it again: the sync runs without asking for the password
      (debug and release x86_64 builds; a page written on the server while the app was stopped came down after the restart)
- [x] `adb shell run-as eu.cdfg.notatnik` and `grep -r` for the password in the app's whole data folder: found nowhere (debug build; a release build refuses `run-as` at all)
- [x] Disconnecting deletes the secret (the file no longer holds that entry); a value that cannot be decrypted makes the
      sync ask for the password, never crash (corrupted the stored value by hand: the sidebar says _Sign in needed_, nothing
      crashes, the bad entry is removed, entering the password in Settings → Sync stores a new one)
- [x] Rust: `Secrets` is still the only seam, `scripts/check.sh` passes (the Linux keyring and `MemorySecrets` paths unchanged)

### AS2 Reaching the server form by touch

- [x] _Switch graph…_ and _New graph on a server…_ can be reached on a phone-sized screen without a keyboard shortcut (the graph switcher at the top of the sidebar → _Add new graph…_ → _New graph on a server…_; no shortcut needed)
- [x] The form's fields are visible above the on-screen keyboard (the `MainActivity` IME padding); typed with Gboard on the
      emulator, Tab moves between the fields
- [ ] A paste and a password manager's fill work in the form's fields (not tried)

**Verify:**

- [x] Playwright, mobile viewport with touch: graph switcher → _Add new graph…_ → New graph on a server → fake server → _Download and open_
      opens the graph (scoped locators, `docs/testing.md` §4; `webdav.spec.ts`, "on a phone"; checked to fail when the flag is off)
- [x] Emulator: the same route, every field typed with the soft keyboard
- [x] Emulator, release build: the shell sets the flag before the page loads (the start screen has no folder field or chooser,
      Settings → Appearance names the app's own storage)

**Found on the emulator, fixed in AS2** (the route works, but these are wrong or confusing on Android):

- [x] The start screen offers _Another folder_ with a path field and _Choose a folder…_, which can do nothing on a phone
      (`pick_folder` answers `None`); on mobile only the graph list and _On a server_ should be there
- [x] Settings → Appearance says "Stored in `~/.config/notatnik/settings.toml`" (and the restore text names the same file):
      on a phone they say "the app's own storage" / "the settings file of this device". The `Ctrl` hints stay: a tablet with a
      keyboard uses them

### AS3 First download over a real network

**Verify:**

- [x] Emulator against the Docker Nextcloud (`http://10.0.2.2:…`): _Check the folder_ finds a graph, _Download and open_ brings
      the pages down, the journal opens, the sidebar shows `☁ Synced …` (the first download took about 9 s on the emulator;
      Nextcloud needs `10.0.2.2:8081` in `trusted_domains`, `dev/nextcloud/setup.sh` now sets it). **Not covered:** a drawing
      in the graph (see AS4)
- [ ] A phone (real, arm64 release APK) against a real Nextcloud over `https://`: the same, which also shows that the
      bundled TLS roots work on the device
- [ ] A wrong password and an unreachable server give the messages of `docs/webdav.md`, not a blank screen

### AS4 Edits both ways, and sync on resume

- [ ] The app starts a sync when it comes back to the foreground (the page's `visibilitychange`), in addition to the
      triggers of SY-6

**Verify:**

- [x] Edit on the phone: the file is on the server within about 10 s while the app is open (20 s, measured, while the editor was still open)
- [ ] Edit on the desktop: visible on the phone within about 10 s of opening the app again
- [ ] The same block edited on both while the phone is offline (airplane mode): after reconnecting, the page merges or
      a conflict copy appears in the Conflicts view; nothing is lost
- [ ] A drawing made on the phone is on the server as an SVG below `assets/drawings/` and opens on the desktop
- [ ] Airplane mode: edits are kept and uploaded after reconnecting, in order; the sidebar says offline, not an error

### AS5 Encryption key (EN-10) and release

- [ ] The "keep the key in the keyring" switch of Settings → Sync → Encryption uses the same Keystore store
- [ ] `docs/phases/phase-8.md`, `docs/webdav.md`, `docs/keymap.md` and `docs/install.md` say what Android does

**Verify:**

- [ ] An encrypted path synced from the desktop is readable on the phone after unlocking, and stays unlocked after a restart when the switch is on
- [ ] A release APK built with `make android-apk` installs and passes AS1 and AS3 on a real device
- [ ] `scripts/check.sh` passes
