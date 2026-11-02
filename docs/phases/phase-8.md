# Phase 8: Android

**Goal:** the same app on your Android phone, focused on capture, journals, search, tasks and
reading, with the graph synced to Nextcloud by the app itself (built-in WebDAV sync).

**Requirements covered:** PL-3, NC-3, AN-1–4, IN-1 (share target), PG-7.
**Depends on:** phases 1–5 (phase 6/7 features are added where they work on mobile).

## Exit criteria

1. Every task checked, with every **Verify** item passing; `scripts/check.sh --full` passes, including the WebDAV sync suite.
2. The app runs on an emulator and on your real phone; a signed APK installs from a file.
3. Two weeks of real use on phone + desktop with zero lost edits (check the sync log and conflict list).

## Milestones

|     | Milestone                 | Tasks       |
| --- | ------------------------- | ----------- |
| 8A  | Toolchain and first build | P8.1–P8.2   |
| 8B  | Built-in WebDAV sync      | P8.3–P8.7   |
| 8C  | Mobile UI                 | P8.8–P8.11  |
| 8D  | Capture and integrations  | P8.12–P8.14 |
| 8E  | Release                   | P8.15       |

## Key design decisions for this phase

| Topic            | Decision                                                                                                                                                                                                                                                                                                                                     |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Storage location | The graph lives in app-private storage (no Android scoped-storage permission issues). An "export graph" action copies it to shared storage if needed.                                                                                                                                                                                        |
| File sync        | Built-in WebDAV sync in `notatnik-graph` behind a `SyncBackend` trait. Desktop keeps using the Nextcloud client by default (NC-1) but _can_ use the built-in sync too.                                                                                                                                                                       |
| Sync algorithm   | Per-file state store: `{path, local_hash, remote_etag, synced_hash}`. Cases decided three-way (see P8.4). Both sides changed → try the block-level merge (P5.1) with the synced version as base; if it's not clean → keep both (write a conflict copy using the same naming pattern Nextcloud uses, so the existing conflict UI handles it). |
| When sync runs   | Android restricts background work: sync on app start/resume, after edits (debounced), on pull-to-refresh, and periodically _while the app is in the foreground_. Optional background sync via WorkManager is a stretch goal (needs a Tauri mobile plugin, written in Kotlin).                                                                |
| Credentials      | Same Nextcloud app password as CalDAV; stored in the Android Keystore (via a Tauri plugin such as stronghold, or a small Kotlin plugin).                                                                                                                                                                                                     |
| Mobile scope     | Full editing is supported, but the UI is optimised for: quick capture, today's journal, agenda, search, reading pages. Heavy views (big tables, timeline) are read-mostly on mobile.                                                                                                                                                         |

---

## 8A: Toolchain and first build

### P8.1 Android toolchain (you)

**Status:** the shell builds for Android on Linux (`scripts/android.sh build`; the Gradle project is in
`src-tauri/gen/android`) and was run in an emulator (a tablet, Android 14, x86_64 image, KVM): it starts, makes the graph
`notes` in its own storage, shows today's journal, a typed line is saved as Markdown, `/draw` opens the drawing editor,
Done stores the SVG below `assets/drawings/` and the picture shows in the note. **Not tried:** a real phone or tablet,
the stylus, Nextcloud sync, the arm64 build on a device, a release build and signing, a phone-sized screen.
Two things the emulator showed and that are fixed: the page was drawn under the status bar (`MainActivity` pads the
content by the system bars) and pictures did not load (Android reaches a custom protocol as `http://asset.localhost`,
not `asset://localhost`; the UI now asks Tauri's `convertFileSrc`). A third one found later: the on-screen keyboard
covered the lower part of the page, so text typed into a field there (the custom date pattern in Settings) could not be
seen; `MainActivity` now adds the keyboard's height to the bottom padding, so the page gets shorter and the field stays
in view (tried in the emulator with Gboard: the field is shown above the keyboard and typed text appears).

`scripts/android.sh doctor` says what is found and what is missing; `scripts/android.sh init | dev | build` run the Tauri
Android commands with the SDK, NDK and JDK it finds (`~/Android/Sdk` by default). The shell is already a library with
the mobile entry point and keeps the device folders and the first graph in the app's own storage (`phases/ipad.md`
has the design; it is the same for Android). Android has no keyring backend yet: a server password is kept for the
session only.

- [ ] Install Android Studio (or the command-line tools), SDK platform + build tools, NDK; set `ANDROID_HOME` and `NDK_HOME`; `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`; a JDK (17+)

**Verify**

- [ ] `cargo tauri info` reports the Android SDK/NDK as found
- [ ] An emulator (x86_64 image) boots from Android Studio's Device Manager

### P8.2 First Android build

- [ ] `cargo tauri android init`; adjust the core and graph crates for Android (paths via the Tauri path API, no hard-coded XDG paths; the file watcher may be unnecessary on Android since all changes come through the app or sync)
- [ ] `cargo tauri android dev` on the emulator

**Verify**

- [ ] The app starts on the emulator, creates a graph in app storage, and editing today's journal works
- [ ] Rust unit tests for `core` pass when cross-compiled (`cargo test` on the host is enough for pure code; a smoke test runs on the device)
- [ ] The same build runs on your real phone over USB (`adb devices` lists it)

## 8B: Built-in WebDAV sync

> **Superseded:** the WebDAV client and the sync engine (P8.3 to P8.7) were built for the desktop (`crates/sync`, SY-1..SY-9). What Android still needs
> (the password in the Keystore, the server form by touch, sync on resume) is in [android-sync.md](android-sync.md).

### P8.3 WebDAV client

- [ ] Using the same HTTP stack as CalDAV: PROPFIND (depth 1, recursive walk), GET, PUT with `If-Match`/`If-None-Match`, DELETE, MKCOL; Nextcloud path `/remote.php/dav/files/<user>/<folder>/`
- [ ] Respect the exclusions: `.notatnik/tmp/`, local-only files

**Verify**

- [ ] Integration tests against the Docker Nextcloud (P4.2): list, upload, download, conditional PUT conflict (412), delete, nested folder creation

### P8.4 Sync engine

- [ ] Decision table implemented and tested exhaustively:

| Local vs synced | Remote vs synced        | Action                                                   |
| --------------- | ----------------------- | -------------------------------------------------------- |
| unchanged       | unchanged               | nothing                                                  |
| changed         | unchanged               | upload (If-Match)                                        |
| unchanged       | changed                 | download (guarded write)                                 |
| changed         | changed                 | merge (P5.1); clean → write + upload; else conflict copy |
| deleted         | unchanged               | delete remote (move to remote `.notatnik/trash/`)        |
| unchanged       | deleted                 | delete local (move to local trash)                       |
| deleted         | changed                 | download (remote edit wins over local delete)            |
| changed         | deleted                 | upload (local edit wins over remote delete)              |
| new             | absent                  | upload                                                   |
| absent          | new                     | download                                                 |
| new             | new (different content) | merge or conflict copy                                   |

- [ ] Renames are treated as delete + create (simple and safe; content is kept either way)
- [ ] Sync is resumable: the state store is updated per file after each successful operation

**Verify**

- [ ] A unit test for every row of the table (fake WebDAV backend), plus the "interrupted in the middle" cases: kill after the upload but before the state update → the next run recognises the remote file as already equal (hash comparison), with no duplicate or conflict copy
- [ ] Property test: random sequences of local/remote edits and syncs on two simulated devices → in the end, both sides are equal, and every edit is present either in the files or in a conflict copy (nothing lost)
- [ ] A deletion never removes content irrecoverably: deleted files are in a trash folder on that side

### P8.5 Initial sync and large graphs

- [ ] First sync: download the whole graph with progress; parallel downloads (limit 4); index built progressively (today's journal first)

**Verify**

- [ ] The synthetic graph (10k pages) from the Docker Nextcloud: the first sync completes (record the time) and the app is usable before it finishes
- [ ] Interrupting the first sync (airplane mode) and resuming → completes without re-downloading finished files

### P8.6 Sync triggers and status

- [ ] Triggers as in the decisions table; a status indicator; sync log screen (shared with the CalDAV log UI)

**Verify**

- [ ] Edit on the phone → the file appears in Nextcloud within ~10 s (debounce + upload) while the app is open
- [ ] Edit on the desktop → visible on the phone after resume or pull-to-refresh
- [ ] Airplane mode → edits are queued; reconnect → all are uploaded, in order

### P8.7 Desktop option

- [ ] Settings on the desktop: "Sync using: Nextcloud client (folder) / built-in WebDAV". Built-in mode must never run on a folder that the Nextcloud client also syncs (warn if a `.sync_*.db` file is found in the folder)

**Verify**

- [ ] Selecting built-in sync on a folder managed by the Nextcloud client → a warning, and the change is refused

## 8C: Mobile UI

### P8.8 Responsive layout (AN-1)

- [ ] Single-column layout below 700 px width; sidebars as drawers; bottom bar: Journal, Search, Agenda, Pages, Capture
- [ ] Touch targets ≥ 44 px

**Verify**

- [ ] Playwright with a mobile viewport (mock backend): the main flows work; axe check passes
- [ ] On the real phone: every screen is usable one-handed (manual)

### P8.9 Touch block editing

- [ ] Tap to edit; a toolbar above the keyboard with indent, outdent, move up/down, TODO, link `[[`, undo
- [ ] Long-press for block selection and actions; swipe right/left on a block = indent/outdent (optional, configurable)

**Verify**

- [ ] On the real phone: write a 10-block nested outline using only the toolbar; the resulting file is correct
- [ ] The software keyboard never covers the block being edited (scroll-into-view works)

### P8.10 Mobile performance

- [ ] Profile with Chrome remote debugging of the Android WebView (`chrome://inspect`)

**Verify**

- [ ] Typing latency < 16 ms on a 500-block page on your phone; opening a page < 500 ms

### P8.11 Plugin compatibility (PG-7)

- [ ] Plugins run in the Android WebView with the same Worker + iframe sandbox; the P6.8 security suite runs on Android

**Verify**

- [ ] Both example plugins work on the phone
- [ ] The P6.8 attack tests pass on Android

## 8D: Capture and integrations

### P8.12 Quick capture (AN-2)

- [ ] Capture screen (opens straight to an input) → appends to today's journal (or a configured inbox page)
- [ ] Home-screen shortcut (app shortcut) to capture; widget as a stretch goal (needs native Kotlin code)

**Verify**

- [ ] From a locked phone → unlock → shortcut → type → save takes under 5 seconds; the block is in today's journal file

### P8.13 Share target (IN-1)

- [ ] Android share sheet: shared text or a URL → capture screen pre-filled (URL → `[title](url)` when the title can be fetched)

**Verify**

- [ ] Share a page from the browser → it lands in today's journal as a Markdown link

### P8.14 CalDAV on Android (AN-4)

- [ ] The phase 4 sync engine runs on Android with the same triggers as file sync; no DAVx⁵ needed for the app itself

**Verify**

- [ ] Complete a task in the app on the phone → the calendar item is completed (check in the Nextcloud web UI)
- [ ] The app and DAVx⁵ (if also installed) don't fight: no duplicates after a week of use

## 8E: Release

### P8.15 Signed APK

- [ ] Release keystore (backed up outside the repo!), `cargo tauri android build`, versioning scheme shared with desktop

**Verify**

- [ ] The signed APK installs on the phone via file transfer; an update installs over the previous version without losing data
- [ ] The release keystore is backed up in 2 places, and its password is stored in your password manager

---

## Manual acceptance checklist

- [ ] Two weeks of phone + desktop use; capture on the phone daily
- [ ] Edit the same page on both while one is offline → the result is merged or a conflict copy is offered; nothing lost
- [ ] Reinstall the app on the phone → after setup, the first sync restores everything

Record: date / commit / notes:
