# Notatnik

Local-first block outliner (Logseq-inspired, own format). Markdown files are the source of truth;
SQLite is a rebuildable cache. Linux desktop first (Tauri 2 + Svelte 5), Android later.

## Docs (read before larger changes)

- `docs/requirements.md`: requirements with IDs (e.g. NS-3, CA-5); reference them in code comments, tests and commits
- `docs/architecture.md`: stack, crate boundaries, data flow, decisions log
- `docs/file-format.md`: on-disk format spec (draft); update it together with parser changes
- `docs/keymap.md`: what the user can do (keys, mouse, menus, settings), kept in step with the UI
- `docs/placeholders.md`: `{{page}}`, `{{title}}`, `{{date}}` ... in templates and queries, and where the app offers them
- `docs/date-formats.md`: display date formats and the custom pattern tokens (device setting)
- `docs/logging.md`: what the app writes to the terminal log, at which level, and what is never logged (`RUST_LOG`)
- `docs/phases/pdf-annotation.md`: a viewer for PDFs in the graph, highlights and comments kept as blocks of a hidden sidecar page `hls/<name>` (PA-1..PA-10)
- `docs/phases/epub-annotation.md`: the same for EPUBs, foliate-js as reader, CFI locations (EA-1..EA-10)
- `docs/phases/web-annotation.md`: clip a web page as a one-chapter EPUB and annotate it with the EPUB reader (WA-1..WA-10)
- `docs/phases/nextcloud-annotation.md`: open and annotate PDFs and EPUBs of the graph's Nextcloud through a device cache, no copy in the graph (NA-1..NA-8)
- `docs/phases/multi-graph.md`: two graphs open at once, notes moved between them, a color per graph (MG-1..MG-6)
- `docs/install.md`: installing on Linux, `scripts/release.sh`, versions
- `docs/testing.md`: how things are verified (test levels, fixtures, check script)
- `docs/TODO.md`: phase overview and status
- `docs/phases/phase-N.md`: detailed tasks for each phase; every task has a **Verify** block, and a task is done only when all of its Verify items pass. Tick items off there

## Layout

- `crates/core` (`notatnik-core`): parser, model, edits, names, query language, and small pure helpers (`logbook`, `mentions`, `repeat`). **No I/O**; must build for `wasm32-unknown-unknown`.
- `crates/index`: SQLite schema, indexing, search, query → SQL.
- `crates/graph`: file I/O, atomic writes, watcher, conflicts.
- `crates/clip` (`notatnik-clip`): clips a web page into a one-chapter EPUB (fetch with the limits and the public-address check, Readability, clean, pictures, zip). No app, graph or UI dependency (`docs/phases/web-annotation.md`).
- `crates/cli`: `notatnik` binary.
- `crates/log` (`notatnik-log`): starts the terminal logger for the three binaries; libraries only use the `log` macros (`docs/logging.md`).
- `crates/app` (`notatnik-app`): the typed API (`dispatch`), page views, markdown renderer, undo, agenda/query commands. TypeScript bindings are generated from it (`scripts/gen-bindings.sh`).
- `crates/dev-server`: HTTP wrapper around `dispatch`; the browser tests run against it.
- `src-tauri/`: Tauri 2 shell (one `invoke_api` command). `ui/`: Svelte 5 + Vite frontend, Vitest unit tests, Playwright tests in `ui/tests`.

## Commands

```bash
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo check -p notatnik-core --target wasm32-unknown-unknown   # core must stay wasm-clean
cargo run -p notatnik-cli -- <args>
scripts/check.sh            # everything, including UI lint/check/unit/browser tests
scripts/check.sh --full     # plus perf targets, benchmarks, UI performance
scripts/check-webkit.sh     # the browser tests listed in ui/tests/webkit.list in WebKit, the engine of the desktop app
scripts/gen-bindings.sh     # after changing Rust API types
scripts/dev.sh              # cargo tauri dev with its own XDG dirs (~/.dev-notatnik), next to an installed app
scripts/release.sh X.Y.Z    # version, checks, .deb into dist/, commit and tag (no push)
cargo tauri build --bundles deb
make help                   # the same as short names: make check, make deb, make install, make release RELEASE=X.Y.Z
```

Node is installed with fnm: `export PATH="$HOME/.local/share/fnm/node-versions/v24.21.0/installation/bin:$PATH"`.

## Rules

- Never build another checkout (a `git worktree`, an old commit) into this repo's `target/`; use `CARGO_TARGET_DIR=...`. Otherwise Cargo reuses artifacts by crate id and the browser tests run against a stale `notatnik-dev-server`.
- Don't start implementing a phase until the user asks; follow the task order and Verify steps in the phase doc.
- Never rewrite a whole user file when one block changed (ST-3); edits are minimal text replacements using source spans.
- Never store anything in the graph folder that can't be derived from or isn't meant to be synced (ST-2).
- Round-trip fidelity is tested with golden files: parse + no-op edit must be byte-identical.
- UI edits: a commit from an editor, and every command that works on the selection or on a block, must go through `PageDoc.applyWith` (evaluated when it reaches the queue front, from the page as it is then) and only for the still-active editor; never write through a stale block id or a page snapshot taken at the key press.
- Tests: scope locators (`getByRole('tree', { name: 'Blocks' })`), and assert the computed style when visibility matters (Playwright counts `opacity: 0` as visible). See `docs/testing.md` §4.
- Svelte scoped CSS prunes selectors it cannot match: a child rendered by `<Self>` or another component, or a sibling reached through `~`/`:has()`, needs `:global(...)` (see the path connector in `Block.svelte`).
- Svelte 5: never write state inside `$derived`, and never read-then-write the same state in an `$effect` (use `untrack`).
- Settings: a setting that applies the moment it is flipped is a `<Switch>` (`ui/src/lib/components/Switch.svelte`, its style is its own because Svelte scopes styles); an option saved with a button, or in a form or list, is a plain checkbox. A long section is split with `SubNav` (Backup, Sync, Calendar); the open part is in `stores/settingsParts.svelte.ts`, and a link that should land on a part calls `goToSettings(section, part)`.
- Several graphs can be open (`docs/phases/multi-graph.md`): the first is slot `""`, the second `"1"`. In the UI everything one graph owns is a `GraphState` (`ui/src/lib/stores/graphState.svelte.ts`); a sidebar section provides its graph through context (`useGraph()`), and the page area shows the *active* graph, which the older modules (`recent`, `favorites`, `icons`, `archived`, `encryption`, `sync`, `bus`, `backend`) answer for. A new per-graph store goes into `GraphState` as a class, with a facade over `active()` only if the page area needs it. In the backend a command that looks at several graphs belongs in `WORKSPACE_COMMANDS` (`crates/app/src/lib.rs`); everything else runs on the state of the slot it names.
- The graph crate needs Rust 1.89 (`File::try_lock` for the graph lock), the rest of the workspace builds with 1.85; `crates/graph/Cargo.toml` says so.
- The user is new to Svelte: keep UI code idiomatic Svelte 5 (runes) and explain non-obvious idioms.
