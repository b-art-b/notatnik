# Phase 0: Environment, project setup and risk spikes

**Goal:** a reproducible dev environment, an empty workspace that builds and passes all checks, and
answers to the biggest technical risks *before* real implementation starts.

**Requirements covered:** none directly (foundation for NF-9).
**Depends on:** nothing.
**Status:** in progress. Done: toolchain, repo, workspace, `check.sh`, docs, spike S2. Open (need you): editor setup (0.3), spike S1 (needs a display), spike S3 (needs your Nextcloud app password), optional S4, manual acceptance.

## Exit criteria

1. `scripts/check.sh` passes on the empty workspace.
2. The first commit exists with docs and skeleton.
3. Spikes S1–S3 each have a findings document with a clear decision.

---

## 0.1 Toolchain

- [x] Rust (rustup, stable) + `wasm32-unknown-unknown` target
- [x] Node.js LTS (fnm) + pnpm
- [x] `tauri-cli` 2.x, `wasm-pack`
- [x] Testing tools: `cargo install cargo-insta --locked` (snapshot review). Optional: `cargo-nextest` (faster test runner)

**Verify**
- [x] `rustc --version && cargo --version && node --version && pnpm --version && cargo tauri --version && wasm-pack --version`
      prints versions (done 2026-10-03: rustc 1.99.0, node 24.21.0, pnpm 12.8.1, tauri-cli 2.12.1, wasm-pack 0.15.0)
- [x] `cargo insta --version` prints a version

## 0.2 System libraries

- [x] `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` (plus already present: `build-essential curl wget file pkg-config sqlite3`)

**Verify**
- [x] `dpkg-query -W libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` lists all five
- [x] `pkg-config --modversion webkit2gtk-4.1` prints a version

## 0.3 Editor setup

- [ ] VS Code extensions: **rust-analyzer**, **Svelte for VS Code**, **Even Better TOML**, **Vitest**, **Playwright Test** (the last three matter from phase 2)
- [ ] Format on save enabled for Rust (rustfmt) and, later, TS/Svelte (Prettier)

**Verify**
- [ ] Opening `crates/core/src/lib.rs` shows rust-analyzer status "ready", with no errors
- [ ] Hover over a std type shows documentation

## 0.4 Repository

- [x] `git init` (branch `main`)
- [x] `.gitignore` (target, node_modules, ui build output, sqlite files)
- [x] Licence decided 2026-10-03: **AGPL-3.0-or-later**
- [x] Add `LICENSE` (full AGPL-3.0 text from gnu.org) and `license = "AGPL-3.0-or-later"` in `[workspace.package]`
- [ ] Decide the licence of `@notatnik/plugin-api` (phase 6). Recommendation: MIT, so plugin authors aren't forced into the AGPL for using the API types
- [x] Commit convention: Conventional Commits with requirement IDs in the body, e.g.
      `feat(core): parse block properties` + `Refs: PR-1, PR-2`
- [ ] First commit: docs + skeleton

**Verify**
- [x] `cargo metadata --format-version 1 | grep -o '"license":"[^"]*"' | sort -u` shows only `AGPL-3.0-or-later`
- [ ] `git log --oneline` shows the first commit
- [x] `git status` is clean after `cargo build` (build output ignored)

## 0.5 Cargo workspace skeleton

Created and verified on 2026-10-03.

- [x] Root `Cargo.toml` (workspace, resolver 3, edition 2024, shared lints: `unsafe_code = forbid`, clippy `all = warn`)
- [x] `rust-toolchain.toml` (stable + rustfmt + clippy + wasm target)
- [x] Crates: `notatnik-core`, `notatnik-index`, `notatnik-graph`, `notatnik-cli` (binary `notatnik`)
- [x] Note: `crates/core-wasm` (wasm-bindgen wrapper) is added in phase 1 (P1.18), so `core` itself never depends on wasm-bindgen

**Verify**
- [x] `cargo build` succeeds with no warnings
- [x] `cargo test --workspace` runs (0 tests, success)
- [x] `cargo clippy --workspace --all-targets -- -D warnings` succeeds
- [x] `cargo fmt --all --check` succeeds
- [x] `cargo check -p notatnik-core --target wasm32-unknown-unknown` succeeds
- [x] `cargo run -p notatnik-cli` prints `notatnik 0.1.0`
- [x] Dependency direction: `cargo tree -p notatnik-core` lists no other workspace crate

## 0.6 Check script

- [x] `scripts/check.sh` with the phase-0 steps from [testing.md §5](../testing.md#5-the-check-script); stops at the first failure and prints which step failed
- [x] `--full` flag reserved for slow suites (no-op for now)

**Verify**
- [x] `scripts/check.sh` exits 0 on the clean workspace
- [x] Add badly formatted code (e.g. `fn  x( ){}`) → script exits non-zero at the fmt step; revert → passes
- [x] Add `let unused = 1;` → script fails at the clippy step; revert → passes

## 0.7 Project docs

- [x] `README.md`, `CLAUDE.md`, `docs/requirements.md`, `docs/architecture.md`, `docs/file-format.md`, `docs/testing.md`
- [x] `docs/TODO.md` (phase index) + `docs/phases/phase-0.md` … `phase-8.md`
- [x] `fixtures/README.md` describing the fixture layout ([testing.md §2](../testing.md#2-fixtures)); fixture content comes in phase 1

**Verify**
- [x] All links between docs resolve (open in the VS Code Markdown preview, or run a link checker such as `lychee docs/`)

## 0.8 Risk spikes (throwaway code)

Spikes answer questions that could change the architecture. The code lives in `spikes/<name>/`, is
**never** imported by product code, and may be deleted afterwards. The *findings* are what matter:
each spike writes `docs/spikes/<name>.md` with the question, the setup, measurements, the decision,
and follow-up changes to architecture.md if any.

### S1: Outliner rendering and editing performance on WebKitGTK

**Question:** Can a Tauri + Svelte 5 page render and edit a long outline fast enough on Linux's
WebKitGTK, using the "rendered blocks + one CodeMirror on the focused block" approach (ED-2, NF-4)?

- [ ] Minimal Tauri 2 + Svelte 5 app with 2,000 hard-coded blocks (nested, with inline formatting, links, a few code blocks)
- [ ] Unfocused blocks rendered as Svelte components; clicking mounts a single CodeMirror 6 instance in that block
- [ ] Arrow-key navigation moves the editor between blocks
- [ ] Measure with `performance.now()` and the WebKit inspector (right-click → Inspect in dev builds):
      initial render, focus switch, keystroke-to-paint latency, memory
- [ ] Repeat with 200 blocks and with list virtualisation (render only visible blocks)

**Verify / pass criteria**
- [ ] Keystroke-to-paint < 16 ms with 2,000 blocks on the page
- [ ] Focus switch between blocks < 50 ms
- [ ] Initial render of 2,000 blocks < 500 ms (or < 100 ms with virtualisation)
- [ ] Findings recorded in `docs/spikes/s1-webkitgtk-outliner.md`, including whether virtualisation is needed from day one
- [ ] If the criteria fail: document the alternatives evaluated (virtualisation, simpler DOM, a Chromium-based shell) and the decision

### S2: Markdown parser choice (`pulldown-cmark` vs `comrak`)

**Question:** Which base parser gives reliable **byte offsets** for list items and inline elements,
handles our extensions, and builds small for WASM (D7, ST-3)?

- [x] Parse 3 sample files (one with CRLF, one with tabs, one with code fences containing `- ` and `[[x]]`)
- [x] Check: offset of each list item, of each inline link/code span; whether we can find `[[…]]`, `((…))` and `#tag` ourselves in text events while skipping code spans
- [x] Alternative evaluated: hand-written line-based outline parser (blocks, properties) + base parser only for inline content inside a block
- [x] Measure: parse time for a 2,000-block file; `.wasm` size (release, `wasm-opt -Oz`) for a minimal parse function

**Verify / pass criteria**
- [x] Decision recorded in `docs/spikes/s2-parser.md` and as a new row in the architecture decisions log
- [x] Chosen approach demonstrably reproduces exact byte spans on all 3 sample files (printed spans sliced from the source match)

### S3: CalDAV round trip with your Nextcloud

**Question:** Do `libdav` + `icalendar` work against your real Nextcloud for the operations phase 4 needs?

- [ ] Create a Nextcloud **app password** (Settings → Security) and a dedicated test calendar, e.g. `notatnik-test`
- [ ] Small Rust binary: discover calendars, list them, PUT a VTODO with a fixed UID, GET it back
- [ ] Modify the task in the Nextcloud Tasks web app (complete it, change the due date)
- [ ] Fetch changes via a `sync-collection` REPORT using the stored sync-token; print changed UIDs and the new ETag
- [ ] PUT with a stale `If-Match` ETag → expect HTTP 412

**Verify / pass criteria**
- [ ] All five operations work; a transcript is saved in `docs/spikes/s3-caldav.md`
- [ ] Any Nextcloud quirks noted (URL layout `/remote.php/dav/calendars/<user>/<cal>/`, date formats, PRIORITY handling)
- [ ] If `libdav` is insufficient: alternatives evaluated (`minicaldav`, hand-written requests with `reqwest` + `quick-xml`) and decision recorded

### S4 (optional): Tauri Android hello world

Only if you want early confidence in phase 8. Requires Android Studio + SDK + NDK (large download).

- [ ] `cargo tauri android init` on a throwaway app; run on an emulator

**Verify**
- [ ] The app starts on the emulator, and a Rust command called from the UI returns a value

## Manual acceptance checklist

- [ ] Fresh clone into another folder → follow README setup → `scripts/check.sh` passes
- [ ] All spike findings read and decisions accepted by you

Record: date / commit / notes:
