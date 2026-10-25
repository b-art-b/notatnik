# Phase 2: Desktop app MVP (outliner)

**Goal:** a Linux desktop app you can use every day for journals and notes: fast keyboard-driven
outliner editing, links, backlinks, namespaces, search, with all edits safely written to Markdown files.

**Requirements covered:** PL-1, PL-4, ED-1–9, ED-13, LN-1–3, LN-5, LN-7, LN-8, NS-3–5, NS-7, JR-1–3,
SR-1, SR-2, ST-4, ST-5 (UI side), NF-4, NF-6, NF-11 (keyboard).
**Depends on:** phase 1 complete; spike S1 findings (virtualisation yes/no).

**Status (2026-10-04):** mostly implemented and tested in a browser against the real backend; not yet used daily.

| Task                                                                                                                                                              | State                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| P2.1 scaffold                                                                                                                                                     | Done: Tauri 2 + Svelte 5 + Vite, lint/check/test pass, `cargo tauri build` makes a `.deb`. Not verified: hot reload in `cargo tauri dev`                                                                                                                                                                                                                                             |
| P2.2 learn Svelte                                                                                                                                                 | yours                                                                                                                                                                                                                                                                                                                                                                                |
| P2.3 typed IPC                                                                                                                                                    | Done with `ts-rs` and one `invoke_api` command (decision D20); stale bindings fail `cargo test`                                                                                                                                                                                                                                                                                      |
| P2.4 mock backend                                                                                                                                                 | Replaced by the HTTP dev backend (D21); the same UI runs on both backends                                                                                                                                                                                                                                                                                                            |
| P2.5 `tauri-driver` e2e                                                                                                                                           | **Open** (needs `sudo apt install webkit2gtk-driver`); browser tests cover the same flows                                                                                                                                                                                                                                                                                            |
| P2.6-P2.9 reading, refs, backlinks, collapse                                                                                                                      | Done and tested (escaping, cycles, backlink counts, collapse persistence). Not verified: external links open the system browser (manual)                                                                                                                                                                                                                                             |
| P2.10 editor on the focused block                                                                                                                                 | Done, including click-to-cursor. Deviation: the `^id` is **hidden** in the editor (kept automatically) instead of shown raw                                                                                                                                                                                                                                                          |
| P2.11 keyboard model                                                                                                                                              | Done; `docs/keymap.md`; logic unit-tested; main keys browser-tested                                                                                                                                                                                                                                                                                                                  |
| P2.12 multi-select                                                                                                                                                | Done and tested (extend, indent, copy as Markdown, delete + undo)                                                                                                                                                                                                                                                                                                                    |
| P2.13 undo/redo                                                                                                                                                   | Done and tested (3-step exact restore, refusal after an external change)                                                                                                                                                                                                                                                                                                             |
| P2.14 autocomplete                                                                                                                                                | Done and tested (`[[`, `((` with `^id` written to the target, `#`, `/`)                                                                                                                                                                                                                                                                                                              |
| P2.15 zoom, P2.16 palette, P2.18 rename UI, P2.19-P2.22 links, sidebar, tree, breadcrumbs, P2.24 dates, P2.25 search                                              | Done and tested                                                                                                                                                                                                                                                                                                                                                                      |
| Page zoom (Ctrl +/-/0, Ctrl+wheel, Firefox steps)                                                                                                                 | Added beyond the plan. Tauri webview zoom in the app, CSS `zoom` in a browser; level in `localStorage`. Unit-tested step logic only; the Tauri path is untested in a real window                                                                                                                                                                                                     |
| Settings additions                                                                                                                                                | Device settings in `settings.toml` `[ui]`: `theme`, `date_locale` (language, 11 offered), `date_format` (`full`/`long`/`short`/`iso`/`custom`) with `date_pattern` ([date-formats.md](../date-formats.md)), `link_brackets` (show `[[ ]]` around rendered links; default off, always raw while editing). Tested in `app.rs` (file isolation, validation) and `reading.spec.ts`       |
| Slash menu redesign, path to the focused block, All pages view, unlinked references, task logbook, ISO calendar date picker                                       | Added beyond the plan (requirements ED-15..ED-18, NS-11, TK-7, LN-4). Browser-tested: `slash.spec.ts`, `path.spec.ts`, `pages.spec.ts`, `navigation.spec.ts` (unlinked), `reading.spec.ts` (settings). Not measured in WebKitGTK                                                                                                                                                     |
| Namespace move (NS-8), group pages by property (NS-12), page-property editor (NS-13), mouse navigation (ED-19), day title link, logbook notes rendered with links | Added beyond the plan. Tested: `graph.rs`, `app.rs`, `props.spec.ts`, `mouse.spec.ts`, `navigation.spec.ts`, `slash.spec.ts`. Mouse back/forward buttons are tested with synthetic DOM events (Playwright cannot press them); not tried with a real mouse in WebKitGTK                                                                                                               |
| Settings page redesign                                                                                                                                            | Four panes (Appearance, Dates & language, Tasks, Graph) with scope badges, segmented controls, a switch, a zoom stepper and example-bearing date formats; applies at once with inline confirmation. Browser-tested in `props.spec.ts`, `reading.spec.ts`, `navigation.spec.ts`                                                                                                       |
| Drag-and-drop reordering, Recent list, Hierarchy section, page Tasks section, rename without subpages, emojis and page icons, property view                       | Added beyond the plan (ED-20..22, NS-15..17, TK-9, NS-14). Tested: `dnd.spec.ts`, `recent.spec.ts`, `hierarchy.spec.ts`, `page-tasks.spec.ts`, `emoji.spec.ts`, `props.spec.ts`, `navigation.spec.ts`, unit tests for `dropMove` and the emoji search, Rust tests for subpages, page icons, rename. Drag-and-drop is HTML5 (Chrome-tested); WebKitGTK in the packaged app is untried |
| Path segments as implicit tags (NS-18)                                                                                                                            | `path:` query clause, `segment_view`/`segment_places` commands, segment view with places, tasks and references, count badge in the Hierarchy. Tested in `app.rs` (every place, prefix is not a segment, placeholders don't count, query clause incl. negation and wildcards) and `segment.spec.ts`                                                                                   |
| P2.17 paste                                                                                                                                                       | Done (unit-tested parsing); no browser test                                                                                                                                                                                                                                                                                                                                          |
| P2.23 journals                                                                                                                                                    | Done (injected clock, no file for empty days); cold-start time (NF-6) not measured                                                                                                                                                                                                                                                                                                   |
| P2.26 save indicator                                                                                                                                              | Done; flush on quit is wired (`beforeunload`) but not tested                                                                                                                                                                                                                                                                                                                         |
| P2.27 external changes                                                                                                                                            | Done and tested (no focus jump, keep mine / take theirs, other block untouched)                                                                                                                                                                                                                                                                                                      |
| P2.28 conflict files                                                                                                                                              | Done and tested (banner appears without restart; side-by-side view)                                                                                                                                                                                                                                                                                                                  |
| P2.29 performance                                                                                                                                                 | Chrome numbers in `docs/benchmarks.md` (216 ms open, ~1 frame typing); WebKitGTK still to measure (S1)                                                                                                                                                                                                                                                                               |
| P2.30 settings                                                                                                                                                    | Done and tested (device vs graph files)                                                                                                                                                                                                                                                                                                                                              |
| P2.31 accessibility                                                                                                                                               | Done: axe finds no serious/critical violations on page view and search; keyboard-only flow tested                                                                                                                                                                                                                                                                                    |
| P2.32 packaging                                                                                                                                                   | `.deb` builds (6.9 MiB). **Open:** install test on Mint, AppImage                                                                                                                                                                                                                                                                                                                    |

## Exit criteria

1. Every task checked, with every **Verify** item passing.
2. `scripts/check.sh` passes, including UI lint, typecheck, Vitest and Playwright (mock backend).
3. `scripts/check.sh --full` passes, including `tauri-driver` end-to-end smoke tests.
4. The manual acceptance checklist passes, and you have used the app as your only notes tool for **one week** without data loss.
5. `.deb` and AppImage build and install on Linux Mint.

## Milestones

|     | Milestone                                       | Tasks       |
| --- | ----------------------------------------------- | ----------- |
| 2A  | Scaffold, typed IPC, mock backend               | P2.1–P2.5   |
| 2B  | Reading: page rendering                         | P2.6–P2.9   |
| 2C  | Writing: the outliner editor                    | P2.10–P2.18 |
| 2D  | Navigation: links, namespaces, journals, search | P2.19–P2.26 |
| 2E  | Safety: external changes and conflicts          | P2.27–P2.28 |
| 2F  | Performance, settings, packaging                | P2.29–P2.32 |

## Key design decisions for this phase

Record each in the architecture.md decisions log when implemented.

| Topic             | Decision                                                                                                                                                                                                                                                                                  |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| IPC typing        | `tauri-specta` generates TypeScript bindings from Rust commands. The UI never calls `invoke("string")` directly.                                                                                                                                                                          |
| UI state          | Svelte 5 runes in `.svelte.ts` modules (`$state`, `$derived`), with one page store per open page, keyed by runtime block IDs (P1.26).                                                                                                                                                     |
| Rendering         | Unfocused blocks are rendered from the parsed AST (from the backend) as Svelte components, **not** via `innerHTML`. This prevents XSS and makes links interactive. Raw HTML is shown escaped.                                                                                             |
| Edit pipeline     | Optimistic: the UI updates immediately, then sends `BlockOp`s with the page's base hash. The backend applies the op (P1.14), writes with a guard (P1.31) and returns the new hash + patches. On `Stale`/`Conflict`, the page reloads and re-applies pending edits where possible (P2.27). |
| When text is sent | Typing stays in CodeMirror; the text is committed on blur, on structural ops, and after 500 ms idle. Ops are queued per page and sent in order.                                                                                                                                           |
| Undo              | CodeMirror's own history handles typing _inside_ the focused block. A global undo stack of `TextPatch` groups (P1.14) handles committed edits and structural ops. Ctrl+Z goes to CodeMirror while it has unsaved history, then to the global stack.                                       |
| Mock backend      | The `api` module has `tauri` and `mock` implementations; mock = WASM core + in-memory fixture graph ([testing.md §4](../testing.md#4-mock-backend-for-ui-tests)).                                                                                                                         |

---

## 2A: Scaffold, typed IPC, mock backend

### P2.1 Scaffold Tauri 2 + Svelte 5 + TypeScript + Vite

- [ ] `src-tauri/` (Rust app crate, part of the Cargo workspace) and `ui/` (Vite + Svelte 5 + TS, pnpm)
- [ ] Use plain Vite + Svelte, not SvelteKit (no server-side rendering or file routing needed; simpler). A small client-side router for page/journal/agenda/search views
- [ ] ESLint + Prettier + `svelte-check`; Tauri capabilities file restricting commands to the main window

**Verify**

- [ ] `cargo tauri dev` opens a window showing "Notatnik" with hot reload working (edit a `.svelte` file → the window updates)
- [ ] `pnpm -C ui lint`, `pnpm -C ui check` (svelte-check) and `pnpm -C ui test` pass
- [ ] `cargo tauri build` produces a binary that starts

### P2.2 Learn Svelte 5 basics (you)

- [ ] Official tutorial sections: basics, reactivity (runes: `$state`, `$derived`, `$effect`, `$props`), events, bindings, lifecycle, `.svelte.ts` shared state
- [ ] Read the CodeMirror 6 "System Guide" (state, view, transactions, extensions)

**Verify**

- [ ] You can explain the difference between `$derived` and `$effect`, and when _not_ to use `$effect`
- [ ] Small exercise: a component that lists items from a `.svelte.ts` store and updates when the store changes

### P2.3 Typed IPC layer

- [ ] Tauri commands (first set): `open_graph`, `get_page`, `apply_ops`, `search`, `search_titles`, `backlinks`, `list_namespace`, `journal_for(date)`, `rename_plan`, `rename_apply`
- [ ] Event: `graph-changed { pages, hashes }` from the watcher (P1.32)
- [ ] `tauri-specta` export to `ui/src/lib/api/bindings.ts`; regenerated by a script

**Verify**

- [ ] Changing a Rust command signature without regenerating → `pnpm -C ui check` fails (the type mismatch is caught)
- [ ] `scripts/check.sh` regenerates the bindings and fails on `git diff` (stale bindings can't be committed)
- [ ] An integration test in Rust calls each command handler directly with a fixture graph

### P2.4 Mock backend

- [ ] `ui/src/lib/api/mock.ts` uses `core-wasm` (P1.18) + an in-memory store loaded from a fixture graph (bundled as JSON at build time)
- [ ] `pnpm -C ui dev:mock` serves the UI in a normal browser

**Verify**

- [ ] `pnpm -C ui dev:mock` → open in Chromium → the `basic` fixture's journal page renders
- [ ] A Playwright test boots the mock and asserts the page title is visible
- [ ] The same UI code runs in both modes; the only difference is the API implementation, selected at build time

### P2.5 End-to-end test harness

- [ ] `sudo apt install webkit2gtk-driver`; `cargo install tauri-driver --locked`
- [ ] `e2e/` with WebdriverIO; starts the built app against a temp copy of a fixture graph

**Verify**

- [ ] The smoke test passes: the app starts, today's journal is visible, typing `hello` into the first block and pressing Esc → the journal file on disk contains `- hello`

## 2B: Reading (page rendering)

### P2.6 Page view

- [ ] Page title (display name), frontmatter properties (collapsible), block tree with bullets and indentation guides
- [ ] Inline rendering from the AST: bold, italic, code, strike, highlight, links, tags, external links (open in the system browser), images from `assets/`
- [ ] Code blocks with syntax highlighting (reuse CodeMirror language packs in read-only mode, or Shiki; decide)
- [ ] Tables (GFM)

**Verify**

- [ ] Playwright (mock): a fixture page with every inline element renders; snapshot of the accessible DOM tree (role/name) matches
- [ ] A block containing `<script>alert(1)</script>`, `<b>bold</b>` and `<img src=x onerror=alert(1)>` renders as literal text (escaped, per the 2026-10-03 decision), and no alert fires
- [ ] Clicking an external link opens the system browser, not the app window (manual check in the Tauri build)

### P2.7 Block refs and embeds (read-only rendering)

- [ ] `((id))` renders the referenced block's content inline; `!((id))` renders the block with children; `![[page]]` renders the page
- [ ] A cycle (A embeds B embeds A) is detected and rendered as an error chip, not infinite recursion
- [ ] A missing target renders as a "broken ref" chip

**Verify**

- [ ] Playwright on fixture `refs`: ref, embed, page embed, cycle and missing cases each render as expected

### P2.8 Backlinks panel

- [ ] Below the page: linked references grouped by source page, each with a breadcrumb and the block (with children collapsed)
- [ ] Count in the header; collapsible

**Verify**

- [ ] On fixture `refs`, the backlinks for 3 pages match `notatnik backlinks --json` output (Playwright test compares counts and texts)

### P2.9 Collapse / expand

- [ ] Toggle on the bullet arrow and with Ctrl+↑/↓; "collapse all" / "expand all" on a page
- [ ] Persisted per device in `state.json` (not in the Markdown file)

**Verify**

- [ ] Collapse a block, restart the app → still collapsed; the Markdown file is unchanged (byte-identical)

## 2C: Writing (outliner editor)

### P2.10 Single CodeMirror on the focused block (ED-2)

- [ ] Clicking a block (or entering it with the keyboard) mounts CodeMirror with the block's **raw source** (first line + properties + continuation lines, without bullet/indentation); Esc or clicking elsewhere unmounts and renders
- [ ] Markdown syntax highlighting in the editor; cursor position kept when entering at the click location

**Verify**

- [ ] Playwright: click in the middle of a word of a rendered block → the editor opens with the cursor at roughly that position (± the hidden syntax length)
- [ ] Typing then Esc → rendered output updates; the file on disk contains the text (e2e smoke)
- [ ] Hidden `^id` is shown in raw mode, and editing it is allowed (power users), but removing an ID that is referenced → warning toast

### P2.11 Keyboard model (ED-3)

Document the full keymap in `docs/keymap.md` and implement it as a command table (keys → command IDs), so the command palette, menus and rebinding later use the same table.

| Keys                              | Action                                                                       |
| --------------------------------- | ---------------------------------------------------------------------------- |
| Enter                             | Split the block at the cursor / new sibling block (in a code fence: newline) |
| Shift+Enter                       | Newline inside the block                                                     |
| Tab / Shift+Tab                   | Indent / outdent (with children)                                             |
| Alt+↑ / Alt+↓                     | Move the block up / down (with children)                                     |
| ↑ / ↓ at the first / last line    | Move the editor to the previous / next visible block                         |
| Backspace at the start of a block | Merge with the previous block (or delete the empty block)                    |
| Delete at the end of a block      | Merge the next block into this one                                           |
| Esc                               | Leave editing → the block becomes selected                                   |
| Ctrl+Enter                        | Cycle task state (from phase 3; reserve now)                                 |
| Ctrl+K                            | Command palette                                                              |

**Verify**

- [ ] Vitest: every command's pure logic (e.g. "indent at index 0 is a no-op", "outdent at the top level is a no-op") has a unit test
- [ ] Playwright (mock): one test per row of the table, asserting the resulting tree
- [ ] After each structural op, the resulting file matches expectations (e2e: 3 representative ops)

### P2.12 Multi-block selection (ED-5)

- [ ] Shift+↑/↓ from an editing or selected block, Shift+click, Esc to clear; selection is always a set of sibling subtrees, rendered with a highlight
- [ ] Operations: copy (as Markdown), cut, delete, indent, outdent, move

**Verify**

- [ ] Playwright: select 3 blocks → Tab → all 3 indented under the preceding sibling; Ctrl+C → the clipboard contains the Markdown of exactly those subtrees
- [ ] Deleting a selection is undoable in one Ctrl+Z

### P2.13 Undo / redo (ED-6)

- [ ] Global stack of patch groups (one group per user action); redo stack cleared on a new action
- [ ] Undo applies inverse patches via `apply_ops` with a base-hash guard; if the file changed externally since → message "can't undo: page changed on disk" (no forced overwrite)

**Verify**

- [ ] Playwright: type in block A, Esc, indent block B, delete block C → three Ctrl+Z restore the exact original; three Ctrl+Shift+Z redo it all
- [ ] e2e: after the undo sequence, the file on disk is byte-identical to the original
- [ ] An external change between the edit and Ctrl+Z → undo refused with a message, file untouched

### P2.14 Autocomplete (ED-7, NS-7)

- [ ] `[[`: fuzzy page search (P1.25), ranking pages in the current namespace and its siblings first; typing `/` after a segment narrows to children; Enter inserts `[[name]]`; an option "create page …" when no exact match
- [ ] `((`: full-text block search → inserts `((id))`, calling `EnsureBlockId` on the target block (which edits the _target's_ file)
- [ ] `#`: tags (same source as pages)
- [ ] `/`: slash commands (insert date, TODO, code block, query block, today/tomorrow …)

**Verify**

- [ ] Playwright: on page `work/projects/alpha`, typing `[[be` lists `work/projects/beta` above `beta-blockers`
- [ ] Typing `[[work/` lists only children of `work`
- [ ] Inserting a block ref → the target file gains ` ^id` on exactly one line, and nothing else in that file changes (e2e)
- [ ] Keyboard-only: ↑/↓/Enter/Esc work in the popup; Esc closes without inserting

### P2.15 Block zoom and breadcrumbs (ED-9)

- [ ] Ctrl+. or clicking the bullet zooms into a block; breadcrumb `page > parent > block` with clickable segments; browser-like back/forward (Alt+←/→)

**Verify**

- [ ] Playwright: zoom into a nested block → only its subtree is shown; clicking the page segment returns; Alt+← goes back

### P2.16 Command palette (ED-13)

- [ ] Ctrl+K: fuzzy search over pages and commands (from the command table, P2.11); recent pages first

**Verify**

- [ ] Every command in the keymap table is reachable from the palette (a test iterates the command table)
- [ ] Ctrl+K → type a page name → Enter → the page opens, in < 100 ms (measured)

### P2.17 Paste and copy basics

- [ ] Pasting multi-line text into a block → split into blocks by lines/bullets (full smart paste comes in phase 5, ED-11)
- [ ] Copying a block (selected, Ctrl+C) → Markdown with children

**Verify**

- [ ] Paste of `- a\n  - b\n- c` → 2 top-level blocks, `b` nested under `a`

### P2.18 Page rename UI (LN-7)

- [ ] Rename from the page title (edit in place) → preview dialog listing the affected files and the count of links (from `rename_plan`) → confirm → apply

**Verify**

- [ ] Playwright (mock): the preview lists the expected files; after confirming, the links on other pages show the new name
- [ ] e2e: files on disk contain the rewritten links; unaffected files are unchanged

## 2D: Navigation

### P2.19 Links and page navigation

- [ ] Clicking `[[link]]` or `#tag` opens the page; Shift+click opens it in the right sidebar; non-existent pages open as empty virtual pages that become files on first content (LN-1)

**Verify**

- [ ] Clicking a link to a missing page → an empty page; typing a block → the file is created at the path from P1.12; leaving without typing → no file created

### P2.20 Right sidebar

- [ ] Open pages or blocks side by side (Shift+click); each panel is editable and closable

**Verify**

- [ ] Editing the same block in the main view and in the sidebar → both update; one file write per commit

### P2.21 Namespace tree (NS-3)

- [ ] Left sidebar tree of namespaces (real and virtual pages), lazy-loaded; counts; expand/collapse persisted; context menu: new child page, rename (single page only until phase 5)

**Verify**

- [ ] On fixture `namespaces`: the tree structure equals the `pages/` folder structure plus virtual parents
- [ ] A new page created via a link elsewhere appears in the tree without reload (via `graph-changed`)

### P2.22 Breadcrumbs and children list (NS-4, NS-5)

- [ ] Namespaced page header: clickable breadcrumb segments
- [ ] Namespace pages show a list of child pages above their own blocks

**Verify**

- [ ] On `work/projects/alpha`, clicking `projects` opens `work/projects` (virtual or real), which lists `alpha` and its siblings

### P2.23 Journals (JR-1, JR-2)

- [ ] Startup opens today's journal; the file is created only on first content
- [ ] Journals view: the last N days in one scroll, loading more on scroll; empty days collapsed into one line

**Verify**

- [ ] With the clock set to a new day (inject the date in tests) → today's journal shows; no file exists until typing
- [ ] Scrolling the journals view loads older days; no file is created for empty days
- [ ] NF-6: cold start to an editable journal < 1.5 s (measured with the warm index, 3 runs, median recorded)

### P2.24 Date links and date picker (JR-3)

- [ ] `[[2026-10-03]]` renders as a nicely formatted date (format configurable); `/today`, `/tomorrow`, `/date` (picker), natural language input via a small parser (`next monday`, `in 3 days`)

**Verify**

- [ ] Vitest table test for natural-language dates with a fixed "today"
- [ ] Inserting `/tomorrow` on 2026-10-03 → `[[2026-10-04]]`

### P2.25 Search UI (SR-1, SR-2)

- [ ] Search dialog (Ctrl+Shift+F or `/` from non-editing mode): results grouped as pages, then blocks, with highlighted snippets; Enter opens the result with the block highlighted and scrolled into view

**Verify**

- [ ] Searching `zolw` in fixture `basic` finds the block with `żółw`; Enter scrolls to it and it flashes
- [ ] Results appear while typing, without visible lag (< 100 ms backend time per query, measured)

### P2.26 Unsaved-state indicator

- [ ] Status indicator: "saved" / "saving" / "error"; on quit with pending edits → flush before exit

**Verify**

- [ ] Type, then immediately close the window → on restart, the text is in the file (e2e)

## 2E: Safety (external changes and conflicts)

### P2.27 External changes while a page is open

- [ ] On `graph-changed` for an open page: reload the tree; preserve runtime IDs (P1.26), focus and cursor
- [ ] If the user is editing a block that changed externally → keep the local text, show a "changed on disk" marker on that block; on commit, the op is `Stale` → offer "keep mine / take theirs / view both"
- [ ] Pending ops for a page whose hash changed → re-validate each op against the new tree (block found with the same content hash → apply, otherwise → conflict prompt)

**Verify**

- [ ] e2e: the app shows page X; an external script appends a block to X's file → the new block appears within 1 s; focus doesn't jump
- [ ] e2e: the user is editing block B; the script changes B's text in the file → the marker appears; choosing "keep mine" writes the local text; choosing "take theirs" discards the local text
- [ ] e2e: the script changes a _different_ block while the user edits B → no prompt; both changes end up in the file

### P2.28 Conflict files visible

- [ ] A banner/badge when Nextcloud conflict files exist (P1.35), with a list of them; opening one shows it read-only next to the original (the full merge tool comes in phase 5)

**Verify**

- [ ] Dropping a conflict file into the graph → the badge appears without a restart

## 2F: Performance, settings, packaging

### P2.29 Performance for long pages (NF-4)

- [ ] Apply the S1 findings: virtualised rendering if needed; memoised block components; one CodeMirror instance
- [ ] Performance test page: the 2,000-block page from the synthetic graph

**Verify**

- [ ] Keystroke-to-paint < 16 ms on the 2,000-block page (measured with the WebKit inspector timeline, 3 runs)
- [ ] Opening the 2,000-block page < 300 ms
- [ ] Memory after opening 50 pages in sequence returns to within 20% of baseline (no leak: check heap snapshots)

### P2.30 Settings

- [ ] Settings screen: graph folder (open/create), theme light/dark/system, date format, week start
- [ ] Device settings in `~/.config/notatnik/settings.toml`; graph settings in `.notatnik/config.toml`

**Verify**

- [ ] Changing a graph setting changes `config.toml` only; changing a device setting changes `settings.toml` only

### P2.31 Accessibility basics (NF-11)

- [ ] All controls reachable by keyboard; visible focus; ARIA roles on the tree (`tree`/`treeitem`) and dialogs

**Verify**

- [ ] Full flow without a mouse: open the app → search a page → edit a block → create a link → open the backlink → back
- [ ] Playwright with `@axe-core/playwright`: no critical violations on the page view and the search dialog

### P2.32 Packaging

- [ ] `cargo tauri build` → `.deb` and AppImage; app icon, desktop entry, app ID

**Verify**

- [ ] `sudo apt install ./notatnik_*.deb` on Mint → the app appears in the menu and starts; uninstall removes it
- [ ] The AppImage runs from the Downloads folder on the same machine
- [ ] User data (settings, cache) lives in `~/.config/notatnik` and `~/.local/share/notatnik`, and nothing is written into the install location

---

## Manual acceptance checklist

- [ ] Write a full day's journal using only the keyboard, without consulting the keymap
- [ ] Create a namespace hierarchy of 3 levels via links; navigate it via the tree and breadcrumbs
- [ ] With the graph in your Nextcloud folder, edit the same page on a second device (or the Nextcloud web editor) while the app is open → the change appears; no data lost
- [ ] Kill the app (`kill -9`) right after typing → after restart, at most the last ~500 ms of typing is lost, and no file is corrupted
- [ ] One week of daily use as your only notes tool: record any friction in `docs/phases/phase-2-feedback.md`

Record: date / commit / notes:
