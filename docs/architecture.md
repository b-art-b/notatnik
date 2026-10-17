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
| D16 | Runtime block identity by tree matching (path+text, then text, then path), session-only | Persisting generated IDs in files | Blocks keep stable UI keys across edits, moves and external rewrites without touching the files (P1.26). |
| D17 | Inline scanner written by hand (links, tags, refs, highlights; skips code spans, escapes and Markdown link targets) instead of pulldown-cmark events | pulldown-cmark for inline | The scanner is ~250 lines and needs no dependency; pulldown-cmark is still the plan for *rendering* emphasis and code spans in the UI (phase 2). |
| D18 | Index resolves links at read time: `refs` store the target's lookup key; aliases/virtual pages are resolved through `keys_for` | Materialised target page ids | No stale ids when an alias or page appears later; virtual pages are garbage-collected after each reindex. |
| D19 | Search text is folded char-by-char (`search_fold`: lower-case, no diacritics, `ł`→`l`) before it goes into FTS5 and into query terms | FTS5's own diacritics removal alone | `unicode61` doesn't fold `ł`; our fold keeps char positions so highlights map back to the original text. |
| D20 | One `invoke_api(name, args)` Tauri command that calls `notatnik_app::dispatch`; TypeScript types and the `Commands` map are generated from the Rust DTOs with `ts-rs` (`scripts/gen-bindings.sh`, checked by a test) | `tauri-specta` with one Rust command per API call | One implementation shared by the desktop shell and the dev server; typed on the UI side; no dependency on a pre-release crate. A stale `bindings.ts` fails `cargo test`. |
| D21 | UI tests run in Chrome against the real Rust backend through `notatnik-dev-server` (HTTP + long-poll), one server and one temp graph per test | A WASM + in-memory mock backend | The mock would need a second implementation of search, backlinks and queries. Real backend tests can assert on files on disk. The `Backend` interface (`ui/src/lib/api`) still has two implementations (HTTP, Tauri). |
| D22 | Undo/redo stack lives in the backend (`AppState`): each `apply_ops` call is one step holding inverse patches and the file hash it expects; an external change refuses the undo | Undo stack in the UI | Guards use the same file hash as writes; page creation is undoable (delete the file); survives UI reloads. |
| D23 | Markdown inside a block is rendered by the backend to a typed AST (`Inline`/`Node`), our syntax is swapped for private-use placeholders before `pulldown-cmark` runs; only http/https/mailto links and local images survive | Rendering HTML strings in the UI | No `innerHTML` anywhere; unsafe schemes and raw HTML are inert. |
| D24 | Page zoom is a per-device view setting: Tauri `setZoom` in the app (capability `core:webview:allow-set-webview-zoom`), CSS `zoom` in a browser; the level is stored in `localStorage`, not in `settings.toml` or the graph | Backend setting in `DeviceSettings` | Needs no Rust/API change and cannot leak into the synced graph (ST-2). Revisit if zoom must follow the device settings file. |
| D25 | A task logbook is a `:LOGBOOK:` ... `:END:` drawer of ordinary continuation lines (`YYYY-MM-DD[ HH:MM] text`), parsed by a pure helper (`core::logbook`) only where it is rendered; the outline parser and block model are unchanged | A new block kind, a `log::` property, or child blocks | Round-trips byte for byte with no parser or snapshot change; properties above it keep working; entries stay readable in any editor. Entries are added in the editor (`/log`), not by backend ops. |
| D26 | Unlinked references: FTS finds candidate blocks, then `core::mentions::find_mention` checks whole-word, case/diacritic-insensitive matches outside `[[ ]]`, `(( ))`, tags, code, URLs, property lines and e-mail addresses; the backend returns the block text with the first mention already linked (`[[Page]]` or `[[Page\|mention]]`) and the UI applies it as a normal guarded `update_text` | A regex over all files; a separate mentions table in the index | Reuses the index and the normal edit/undo path; nothing new to keep in sync. Aliases are matched by their folded keys. |
| D27 | Every segment of a page name is an automatic, indirect tag: `path: Renewal` is a query clause (a `LIKE` on `'/' \|\| key \|\| '/'`, whole segments, wildcards escaped) and the segment view groups the matches by where the segment occurs. A top-level placeholder page (only referenced, e.g. by `#renewal`) with nothing below it is not a place; a linked-only page inside a path is | A `page_segments` table; materialising segments as tags | No schema change, always consistent with the page names (a rename changes the result at once); 14k pages scan in a few ms. A table can be added if graphs grow far beyond 10k pages. |
| D28 | Page icons are a graph setting: `[[page_icons]]` rules (`key`, `value`, `icon`) in `config.toml` plus a page's own `icon:` property, resolved by the backend (`page_icons`) into one `name → emoji` map that the UI keeps and refreshes when pages change | A per-device setting; storing the icon in each page | The meaning of `type: person` belongs to the graph and must sync with it; nothing is written into pages. |
| D29 | Commands that act on the current selection or on a block (indent, outdent, move, delete, editor commits) build their ops in `PageDoc.applyWith`, when the request reaches the front of the page's queue, never from the page as it was at the key press | Computing the ops first and queueing them | A key pressed right after another command otherwise uses a stale page (found as a 1-in-10 flake and as a data-safety bug). |
| D30 | Dragging blocks is plain HTML5 drag and drop on the bullet. `dropMove(from, to, zone)` (pure, unit-tested) turns a drop into one `move` op; paths are those of the page now, because the backend resolves the destination before it removes the block | A drag library; own pointer handling | Little code, native auto-scroll and drag image; undo comes with the op. Moving between pages is not built. Tauri's own file-drop handler swallows HTML5 drag events in the webview, so the window sets `dragDropEnabled: false` in `tauri.conf.json`; without it dragging works in a browser but not in the packaged app (kanban cards, timeline events, bullets). |
| D31 | A link or tag applies to the subtree of its block in queries (`links:`/`tag:`, hence "Mentioning this page" and the agenda tag filter); `direct: yes` opts out. Computed at query time with a recursive CTE over `blocks.parent_id` (new index; schema version 3) | A materialised `inherited_refs` table; copying links onto descendants at ingest | No second source of truth to keep in step, index stays small; the query cost is a few ms (`tag: urgent` 9 → 19 ms at 14k pages). Dates and plain properties are deliberately not inherited. Linked references show the whole block subtree instead of one line. |
| D32 | One component (`SourcedTask`) renders a task listed outside its note: the source note is a right-aligned chip in the task's first row (grid, two columns), not a line above the task. Parent blocks go in the tooltip as plain words (`sourceTitle`); the date is not repeated (the task shows its own schedule lines) | A source line above the task (it read as the footer of the previous task); grouping by page only | Gestalt proximity: an element in the same row can belong to one task only; one component keeps the agenda sections and the page's Tasks section consistent (chip text: `sourceLabel`; own tasks show their parent blocks). (The "due today" card above today's journal was removed again: due tasks live in the agenda.) |
| D33 | `inherit: key, ...` makes a block property apply to its subtree in queries; the nearest block with the key decides and the page's frontmatter is the root. Compiled as a recursive CTE seeded from the property rows (`inherited_prop_sql`); children with their own value start a new chain | Always inherit; copy the value onto children in the file; inherit in the render | Opt-in keeps `deadline`/`status` predictable and the files untouched; the same mechanism as link inheritance (LN-11) | 
| D34 | Journal pages get a derived page property `type: journal` (an index row written at ingest when the frontmatter has no `type`; schema version 5). The journal's calendar glyph goes through the normal icon lookup (own `icon:`, then rules, then the drawn calendar) | A special case in the UI for journals; writing `type: journal` into every day file | Journals are pages: one icon/type mechanism, nothing added to the user's files (ST-2). The type grouping and `page_property_keys` already skip journals, so the sidebar is unchanged. Today's journal may have no file (no index row): `page_icons` adds it explicitly when a journal rule exists. |
| D35 | Compact task details are a device setting (`task_details` in `settings.toml`) applied per call site: `BlockList` (pages, journals, embeds) passes `compact`, other lists do not. One `details` snippet in `Block.svelte` renders the chips and logbook either inline or inside the indicator popover | A graph setting; hiding the details with CSS only | Per-device taste and touch screens differ; a popover (no layout shift) keeps touch and keyboard access; the same markup means the date picker and logbook keep working | 
| D36 | *Block properties* is its own device setting (`block_properties`), next to `task_details`; both feed the same indicator group and popover in `Block.svelte` (`taskCompact`, `propsCompact`). The indicator is a count (`⋯ 2`) with the `key: value` list as tooltip; which properties count is `shownProps` (shared with the card) | One combined "compact details" switch; showing values as chips in the row | Tasks and properties are different tastes; a count is the quietest start and the card stays one hover away. Values-as-chips can be added inside the same group if the count hides too much. |
| D37 | Backups are zip snapshots written by `notatnik-graph` (`backup.rs`) into a device-level folder outside the graph; a thread started at graph open takes them automatically (device settings read at each check) | Git history of the graph, a database dump, a plain folder mirror | One file per moment is easy to copy, verify and restore by hand with any unzip tool; no new format to keep compatible. The zip comment holds a fingerprint (path, size, mtime) so an unchanged graph is not saved again. Git was left out: libgit2 on Android and pruning. A snapshot is never inside the graph folder (ST-2); retention is 7 days, 4 weeks, 6 months. Later additions (BK-7..BK-9): the compression is a per-snapshot choice (deflate or stored, still plain zip, no zstd or tar so that any unzip tool restores); the device settings and the graph's `state.json` ride along under `.notatnik-device/`; an optional second folder is brought up to date by copying missing snapshot files (never created, so an unmounted disk cannot fill the system disk) instead of writing twice, which makes catching up after an absence the same code as the normal case; every written zip is read back before it is kept. |
| D38 | Multi-file operations write a record to `<data dir>/journal/pending.json` before their first step and remove it when they return (`journal.rs`); a record found at open is rolled back with the existing revert code, or dropped when the operation had completed | A general write-ahead log for every page write; two-phase commit in the files | A single page write is already atomic (temp file, fsync, rename) and hash-guarded, so a log there would only add a second source of truth. Only operations of several files can stop half way, and they can already be reverted from their plan; the record makes that survive a crash. |
| D39 | Page history is a per-file folder of recorded versions in the device data dir (`versions.rs`), written where the merge-base copy in `history/` is updated | Git in the graph, a SQLite table of versions, a version per keystroke | One hook sees every app write and every outside change; plain files are inspectable; thinning keeps it small. Writes made by the app within a minute replace each other, restores and outside changes never do, so nothing a user replaced on purpose is lost. |
| D40 | Encryption protects what leaves the device (backups now, sync later), not the local graph. A vault of two secrets (data key, X25519 key) is wrapped by a password slot (Argon2id) and a recovery slot; backups are sealed to the public key so they need no password; each sealed file carries the vault descriptor. Crypto lives in the pure crate `notatnik-crypt` (no I/O; RustCrypto primitives and BLAKE3) | Encrypting inside the graph folder; one vault file; filesystem encryption; backups encrypted with a symmetric key held in memory | Joplin's model, adapted: keys wrapped by the password so a change touches no data, a key ID in every file. Unattended backups were the deciding point: a symmetric scheme would need the password at every start, public-key sealing needs it only to read. Details and the sync plan: [encryption.md](encryption.md). |
| D41 | Pasted and dropped images are stored by the backend (`save_asset`: bytes as base64, sniffed from the content, hashed) in `assets/YYYY/MM/<name>-<hash8>.<ext>` and linked with a path relative to the page file; the editor only sends the bytes and inserts the returned Markdown. The desktop shell serves them over `asset://` | Storing in a folder per page; absolute or graph-root-relative links; re-encoding images | Relative links render in any Markdown tool and in the Nextcloud web viewer; a month folder survives page renames; the hash makes duplicates impossible and names stable across devices. A page move rewrites its relative paths. The app never deletes an asset (undo, sync, other pages may still use it). |
| D42 | A graph on a WebDAV server is a local replica folder plus a sync engine (`notatnik-sync`: a `Remote` trait, a WebDAV client on `ureq` 2, a state file beside the replica, a pure planner, an engine that uploads under `If-Match`). The app, index and watcher only see the replica. Conflicts keep the server's version and save the local one as a "conflicted copy"; folder ETags are trusted only after a probe; passwords live in the keyring behind a `Secrets` trait | Opening files over the network; a virtual filesystem; the Nextcloud desktop client; CRDT merging; `sync-collection` (not offered for files) | Offline use, the index and all existing features stay unchanged; the engine is testable against an in-memory server. `ureq` 3 refuses WebDAV methods, `ureq` 2 allows them. Details: [webdav.md](webdav.md). |

## 10. Known risks

| Risk | Mitigation |
|---|---|
| WebKitGTK (Tauri's Linux webview) performance and quirks | Profile early with a large synthetic graph; keep DOM small (virtualised lists, single CodeMirror). |
| Round-trip fidelity bugs that corrupt user files | Golden-file and property tests (parse → no-op edit → byte-identical); backups in `history/`. |
| Nextcloud sync races (writing while the client uploads) | Atomic writes, hash checks before writing, conflict detection. |
| CalDAV edge cases (timezones, recurrence, server quirks) | Test against a real Nextcloud in Docker; start with VTODO + date-only. |
| Android file access and background sync | Built-in WebDAV sync; graph in app storage. |
| Scope creep | Strict MVP list in [TODO.md](TODO.md); plugins for non-core features. |
