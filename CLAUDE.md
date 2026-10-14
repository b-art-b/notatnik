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
- `docs/testing.md`: how things are verified (test levels, fixtures, check script)
- `docs/TODO.md`: phase overview and status
- `docs/phases/phase-N.md`: detailed tasks for each phase; every task has a **Verify** block, and a task is done only when all of its Verify items pass. Tick items off there

## Layout

- `crates/core` (`notatnik-core`): parser, model, edits, names, query language, and small pure helpers (`logbook`, `mentions`, `repeat`). **No I/O**; must build for `wasm32-unknown-unknown`.
- `crates/index`: SQLite schema, indexing, search, query → SQL.
- `crates/graph`: file I/O, atomic writes, watcher, conflicts.
- `crates/cli`: `notatnik` binary.
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
scripts/gen-bindings.sh     # after changing Rust API types
cargo tauri build --bundles deb
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
- The user is new to Svelte: keep UI code idiomatic Svelte 5 (runes) and explain non-obvious idioms.
