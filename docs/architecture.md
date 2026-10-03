# Architecture

Status: **draft** · Last updated: 2026-10-03

## 1. Tech stack

| Layer | Choice | Rationale |
|---|---|---|
| App shell | **Tauri 2** | Rust backend with a webview UI; small binaries; same codebase targets Android. Electron has no mobile story. |
| UI framework | **Svelte 5** (runes) + **TypeScript** + Vite | Fine-grained reactivity (an outliner has thousands of blocks); easiest to learn; large ecosystem. |
| Block editor | **CodeMirror 6**, one instance mounted on the focused block | Raw-Markdown-while-editing (Logseq style); fast; extensible (autocomplete, syntax highlight). |
| Markdown parsing | Hand-written outline parser + **pulldown-cmark** for inline content only (D15) | Outliner structure, properties and our inline extensions. Compiled natively and to **WASM** so UI and backend share one parser. |
| Index / cache | **SQLite** via `rusqlite`, **FTS5** (with `unicode61 remove_diacritics`), **sqlite-vec** | One embedded DB for structure, full-text and vectors. Rebuildable. |
| File watching | `notify` (+ debouncer) | External changes from Nextcloud, editors and git. |
| CalDAV | `libdav` + `icalendar` crates (wrapped in our `caldav` crate) | Two-way task/event sync with Nextcloud. |
| WebDAV (later) | `reqwest_dav` or `libdav` | Built-in file sync for Android. |
| Secrets | `keyring` crate (desktop); Tauri stronghold / Android Keystore (mobile) | Nextcloud app passwords. |
| Plugins | Web Workers + sandboxed iframes (TS); later **Extism** (WASM) | Sandboxed, cross-platform. |
| AI | **Ollama** HTTP API (local or home server); provider trait for others | Local-first, no Python in the core. |
| Testing | `cargo test`, `proptest`, `insta`, `criterion`; Vitest; Playwright against a mock backend; WebdriverIO + `tauri-driver` for end-to-end; Radicale for CalDAV | See [testing.md](testing.md) |

## 2. Repository layout

```
notatnik/
├── Cargo.toml                 # workspace
├── crates/
│   ├── core-wasm/             # wasm-bindgen wrapper around core (for the UI and mock backend)
│   ├── core/                  # pure logic, no I/O, compiles to wasm32
│   │   ├── parser/            #   Markdown → block tree (with source spans)
│   │   ├── model/             #   Page, Block, Property, Link, Task types
│   │   ├── edit/              #   block operations → minimal text edits
│   │   ├── names/             #   page name ⇔ path, namespace resolution
│   │   └── query/             #   query language parser → query AST
│   ├── index/                 # SQLite schema, indexer, query AST → SQL, search
│   ├── graph/                 # graph service: file I/O, atomic writes, watcher, conflict detection
│   ├── caldav/                # CalDAV client wrapper, block ↔ VTODO mapping, sync engine
│   ├── ai/                    # embedding pipeline, provider trait, retrieval
│   └── cli/                   # `notatnik` CLI: index, query, check, export (dev + power users)
├── packages/plugin-api/       # @notatnik/plugin-api TypeScript package (phase 6)
├── fixtures/                  # test graphs and golden edit cases
├── e2e/                       # WebdriverIO + tauri-driver end-to-end tests
├── dev/                       # dev infrastructure (Nextcloud docker compose, scripts)
├── scripts/check.sh           # the single pre-commit check
├── src-tauri/                 # Tauri app: commands, events, plugin host, settings
├── ui/                        # Svelte app
│   ├── src/lib/editor/        #   outliner, CodeMirror integration, keybindings
│   ├── src/lib/views/         #   list, table, kanban, timeline
│   ├── src/lib/plugins/       #   plugin runtime + public API types
│   └── src/routes/…           #   page, journal, agenda, search, settings
└── docs/
```

Rule: `core` never touches the filesystem, network or clock directly. Everything else depends on
`core`, not the other way round. This keeps the logic testable and shareable with the UI (WASM).

## 3. Data flow

### 3.1 Editing a block (UI → disk)

```
UI keystroke ─► CodeMirror (local state, instant)
   │ debounce ~300 ms / on blur / on structural op (indent, move, new block)
   ▼
Tauri command  apply_ops(page, [BlockOp])
   ▼
graph:  load current file text + its hash
        if hash != hash the UI edited against → external change → 3-way merge or conflict
   ▼
core::edit:  BlockOp → minimal TextEdit(s) on the source (using spans)   ◄── ST-3 fidelity
   ▼
graph:  atomic write (.notatnik/tmp/ → rename), remember new hash
   ▼
index:  reparse this one file, diff blocks, update SQLite in one transaction
   ▼
event "graph-changed" {pages, blocks} ─► UI: live queries / backlinks refresh
```

### 3.2 External change (Nextcloud → disk → UI)

```
notify event ─► debounce ─► ignore if hash == our last write
   ▼
reparse + reindex file ─► event "graph-changed"
   ▼
UI: if the changed page is open and a block is being edited → keep the local edit, mark it as diverged,
    merge on next save (3-way using history/ copy as base)
```

### 3.3 Startup

1. Open graph, then open the index. Compare each file's mtime and size with the index, and reindex only stale files.
2. Show today's journal as soon as its page is indexed (do not wait for full index).
3. Background: embeddings (if AI is enabled), CalDAV sync.

## 4. Index schema (sketch)

```sql
files(path PK, page_id, mtime, size, hash)
pages(id PK, name UNIQUE COLLATE NOCASE, title, ns_parent_id, is_journal, journal_date, is_virtual)
blocks(id PK, page_id, parent_id, ord, block_uid NULL, content, task_state NULL,
       scheduled NULL, deadline NULL, priority NULL, span_start, span_end)
props(owner_kind, owner_id, key, value_text, value_num, value_date)    -- page and block props
refs(src_block_id, kind /*page|tag|block|embed*/, target_page_id NULL, target_block_uid NULL, raw)
aliases(alias COLLATE NOCASE, page_id)
fts_blocks USING fts5(content, tokenize='unicode61 remove_diacritics 2')
vec_blocks USING vec0(embedding float[N])                             -- later
cal_items(uid PK, kind /*event|todo*/, calendar, etag, dtstart, dtend, summary, raw_ics, block_uid NULL)
```

Namespace subtrees are queried with recursive CTEs on `ns_parent_id`, or with a materialised
`ns_path` prefix (`name LIKE 'work/%'`). Pick whichever benchmarks better.

## 5. Query engine

`core::query` parses the query block into an AST (shared with the UI via WASM for syntax errors and
autocomplete). `index` compiles the AST to parameterised SQL. Results are block or page IDs plus the
fields the view needs. Live queries: on every `graph-changed` event, the UI re-runs visible queries.
Later optimisation: only re-run queries whose dependencies (tags, namespaces, properties) intersect
the change set.

Views receive `{rows, columns, groups}` and are plain Svelte components behind a `View` interface, the
same one plugins implement.

## 6. CalDAV sync engine

- Per calendar: store `sync-token` and the ETag for each item.
- **Pull:** REPORT sync-collection, which returns changed and deleted items.
  - Events go into `cal_items`.
  - For a VTODO with UID `notatnik-<graph>-<blockid>`, compare it with the block: changed fields (status,
    due, dtstart, summary, priority) are written to the Markdown block through the normal edit path.
  - A VTODO created outside the app (e.g. on the phone) can be imported into an "inbox" page (configurable).
- **Push:** dated task blocks whose content differs from the last pushed state are PUT with If-Match
  ETag. A 412 response means a conflict, so pull first, then resolve per field (last-modified wins), then log it.
- Mapping state lives in the files (UID derived from the block ID). Per-device state (ETags, tokens)
  lives in the cache and can be rebuilt by a full sync.

## 7. Plugin architecture

```
Plugin bundle (.notatnik/plugins/<id>/manifest.json + main.js [+ ui.html])
   main.js  ─► Web Worker  ── postMessage RPC ──► Plugin host (UI) ──► Tauri commands
   ui.html  ─► sandboxed <iframe> (views, panels)    ▲ permission checks here
```

- The API surface is a versioned TypeScript package (`@notatnik/plugin-api`).
- Capabilities: `graph:read`, `graph:write`, `net:<domain>`, `calendar`, `ai`.
- User scripts (PG-5) use the same runtime with an implicit manifest.
- Later: Extism host in Rust for WASM plugins that need speed or other languages.

## 8. AI pipeline

- `ai` crate: `Provider` trait (`embed`, `chat`), with Ollama as the first implementation.
- Chunking: one block (with parent breadcrumb as context) is one chunk. Re-embed on content change.
- Retrieval: hybrid search (FTS5 BM25 + vector KNN, reciprocal rank fusion).
- Chat answers cite block IDs, which render as `((refs))`.
- Namespace exclusions are enforced in the retrieval layer.

## 9. Key decisions log

| # | Decision | Alternatives considered | Reason |
|---|---|---|---|
| D1 | Files are the source of truth; SQLite is only a cache | DB-first (Logseq DB style) | Nextcloud syncs files well and corrupts or conflicts on DB files; data ownership. |
| D2 | Tauri 2 | Electron, Flutter, native GTK | Android target from the same code; Rust core; small footprint. |
| D3 | Svelte 5 | SolidJS, React | Learning curve (new to both), performance, ecosystem. |
| D4 | Own file format (not Logseq-compatible) | Logseq-compatible | Freedom: folders for namespaces, `^id` block IDs, fenced query blocks. |
| D5 | Namespaces = folders | Logseq `a___b.md` flat files | Browsable in Nextcloud and file managers. |
| D6 | Query language = simple clause DSL compiled to SQL | Datalog (DataScript), SQL exposed directly | Easy to type in a block; safe; fast on SQLite. |
| D7 | Shared Rust parser via WASM | Separate JS parser in UI | One source of truth for syntax; no drift. |
| D8 | Calendar UID derived from block ID | Mapping table in DB | Works across devices, survives cache rebuilds. |
| D9 | No Python in core | Python sidecar for AI | Android packaging; Ollama covers AI needs. |
| D10 | All links are global, no relative links | `[[./child]]`, `[[../sibling]]` | Simpler mental model; a link means the same thing on every page. |
| D11 | `ns: x` matches `x` and everything below it | Wildcards (`x/*`), direct children only | Easiest to type; `page: x` covers the single-page case. |
| D12 | Repeating tasks log completions in a hidden `completed::` property | Child blocks, journal entries, no log | One line, invisible in normal view, queryable (e.g. by a habit tracker). |
| D13 | Raw HTML is escaped, never rendered | Sanitised HTML rendering | Safety (no XSS through notes or synced files) and simplicity. |
| D14 | Licence AGPL-3.0-or-later | GPL-3.0, MIT/Apache-2.0 | Keeps all forks open, including hosted ones. |
| D15 | Hand-written outline parser + pulldown-cmark for inline only (spike S2) | pulldown-cmark/comrak for the whole file | Exact spans for tabs/CRLF; 70x faster; no CommonMark list quirks. See [spikes/s2-parser](spikes/s2-parser.md). |

## 10. Known risks

| Risk | Mitigation |
|---|---|
| WebKitGTK (Tauri's Linux webview) performance and quirks | Profile early with a large synthetic graph; keep DOM small (virtualised lists, single CodeMirror). |
| Round-trip fidelity bugs that corrupt user files | Golden-file and property tests (parse → no-op edit → byte-identical); backups in `history/`. |
| Nextcloud sync races (writing while the client uploads) | Atomic writes, hash checks before writing, conflict detection. |
| CalDAV edge cases (timezones, recurrence, server quirks) | Test against a real Nextcloud in Docker; start with VTODO + date-only. |
| Android file access and background sync | Built-in WebDAV sync; graph in app storage. |
| Scope creep | Strict MVP list in [TODO.md](TODO.md); plugins for non-core features. |
