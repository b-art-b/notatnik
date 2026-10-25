# Roadmap

Each phase has a detailed document with tasks, **Verify** steps for every task, exit criteria and a
manual acceptance checklist. The checklists live **only** in the phase documents; this file is the
overview. How verification works in general: [testing.md](testing.md).

Legend: ⬜ not started · 🟨 in progress · ✅ done

| Phase | Document                                             | Goal                                                                                                                                                                            | Status                                                                                                                                |
| ----- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 0     | [Environment, setup, risk spikes](phases/phase-0.md) | Toolchain, buildable empty workspace, check script; spikes on WebKitGTK performance, parser choice, CalDAV                                                                      | 🟨 (S2 done; S1, S3 and editor setup need you)                                                                                        |
| 1     | [Core without UI](phases/phase-1.md)                 | File format frozen; parser, edit ops, query language, SQLite index, graph service (atomic writes, watcher), CLI, benchmarks                                                     | 🟨 (all tasks done and verified; waiting for your spec review and manual acceptance)                                                  |
| 2     | [Desktop app MVP](phases/phase-2.md)                 | Tauri + Svelte outliner: editing, keyboard model, undo, links, backlinks, namespaces, journals, search, packaging                                                               | 🟨 (implemented and browser-tested; see the status table in the phase doc)                                                            |
| 3     | [Tasks, queries, views](phases/phase-3.md)           | Task states and dates, agenda, live query blocks, list/table/kanban/timeline views, repeating tasks                                                                             | 🟨 (implemented and browser-tested; manual acceptance open)                                                                           |
| 4     | [Calendar (CalDAV)](phases/phase-4.md)               | Two-way task sync and event display with Nextcloud, safe conflict/deletion handling                                                                                             | 🟨 (built and tested against the fake server, Radicale and a real Nextcloud; the compatibility matrix and manual acceptance are open) |
| 5     | [V1 polish](phases/phase-5.md)                       | (P5.8 unlinked references already built) 3-way merge + conflict UI, multiple graphs, assets, aliases, unlinked refs, namespace moves, templates, drag-drop, smart paste, themes | ⬜                                                                                                                                    |
| 6     | [Plugins and scripting](phases/phase-6.md)           | Sandboxed TypeScript plugin API with permissions, custom views, user scripts, example plugins, security suite                                                                   | ⬜                                                                                                                                    |
| 7     | [AI (optional)](phases/phase-7.md)                   | Ollama provider, embeddings in sqlite-vec, hybrid search, related notes, chat with citations, privacy guarantees                                                                | ⬜                                                                                                                                    |
| 8     | [Android](phases/phase-8.md)                         | Tauri Android build, built-in WebDAV sync, mobile UI, quick capture, share target, CalDAV on mobile                                                                             | ⬜                                                                                                                                    |

## Milestone summary

| After phase | You can…                                                                                              |
| ----------- | ----------------------------------------------------------------------------------------------------- |
| 1           | Keep notes in a text editor and use the `notatnik` CLI to search, query, rename and check them safely |
| 2           | Use the desktop app daily for journals and notes                                                      |
| 3           | Manage tasks and project boards in the app                                                            |
| 4           | See and complete your tasks from the phone's calendar/tasks apps and Nextcloud                        |
| 5           | Use a complete V1 desktop app                                                                         |
| 6           | Extend the app yourself                                                                               |
| 7           | Search and ask questions by meaning                                                                   |
| 8           | Use the app on Android                                                                                |

## Built beyond the phase plans (2026-10)

Done while using the app, each with requirement IDs in [requirements.md](requirements.md) and tests; the per-feature status is in the tables of
[phase-2.md](phases/phase-2.md) and [phase-3.md](phases/phase-3.md), how to use it in [keymap.md](keymap.md):

- Editor: slash menu (grouped, searchable), date pickers with an ISO calendar, `/log` task logbook, emojis (`:shortcode`, `/emoji`), drag-and-drop reordering, path to the focused block, page zoom, Firefox-style mouse navigation.
- Pages: Hierarchy, page Tasks section, Recent list, All pages, page properties (edit, group the sidebar by a property, click a property to list its pages), page icons, unlinked references, namespace move/rename (with revert), path segments as implicit tags (`path:`). Renaming a middle path segment renames the namespace (NS-19). Linked references show the block with its sub-bullets (LN-10); links and tags apply to the subtree in queries, `direct: yes` opts out (LN-11, D31). Agenda has a Later section.
- Calendar (phase 4, see [phase-4.md](phases/phase-4.md) and [keymap.md](keymap.md)): push and pull of tasks (CA-4..CA-9), several task calendars with `cal::` (a Calendar chip changes it), tasks without a date, events with repeats and time zones, automatic and paused sync, a sync log with "use the other value", Radicale and Nextcloud test suites (`dev/nextcloud/setup.sh`), the **calendar mode of the Agenda** (day, week, month; drag, resize, tray, rolled-over display; CV-1..CV-9), and copies of other apps' tasks in a journal section that are made once across devices (CA-12).
- Settings: four panes (device vs graph), date language/format/custom pattern, link brackets.
- Several graphs open at once in the backend, taking notes between graphs and a color per graph are built and tested ([phases/multi-graph.md](phases/multi-graph.md), MG-1 to MG-5); the second graph's panel in the UI (MG-6) is being redesigned.
- Encryption: [encryption.md](encryption.md) (EN-1..EN-27). Encrypted backups are built (P0 to P2). Encrypting chosen paths on a WebDAV server is built and tested ([phases/encrypted-sync.md](phases/encrypted-sync.md), E1 to E6; the manual checklist is open); hidden file and folder names and re-encrypting with a new key are built ([phases/hidden-names.md](phases/hidden-names.md), EN-25, EN-26); everything-encrypted with a list of exceptions is built ([phases/encrypt-everything.md](phases/encrypt-everything.md), EN-27); the encrypted replica for folder tools is on hold.
- Sync with a WebDAV/Nextcloud server is built (plain files; SY-1..SY-8, [webdav.md](webdav.md)); encryption of chosen paths on the server is built (see above); Android background sync is open.
- _Forgot the password…_ for graph and backup encryption (2026-10-20, EN-22, E11).
- _Encrypt everything…_ in Settings → Sync → Encryption (2026-10-20, EN-21, E10).
- `notatnik verify` (2026-10-20, EN-20): compares the server copy, decrypted, with the graph folder ([phases/encrypted-sync.md](phases/encrypted-sync.md) E9).
- Pages and lists (2026-10-19/20): _+ Add subpage_ in the Hierarchy (NS-24); archive, unarchive and delete from the All pages rows, the 🔒 of encrypted pages and the _Only encrypted_ filter in All pages, the sidebar tree and Recent (NS-25); connecting a local graph to a server in place (SY-9) and encrypting before the first upload (EN-19, [phases/encrypted-sync.md](phases/encrypted-sync.md) E8).
- Still open from the plans: Phases 4-8, WebKitGTK measurements, AppImage, `tauri-driver` end-to-end tests, the manual acceptance checklists, moving blocks between pages, image attachments other than pictures (P5.5; the unused-images list is built; paste and drop of images are built).

## Known issues

- Fixed 2026-10-19: the UI e2e flake in which the first request of the app got no answer (a few percent of runs, hidden by `retries: 2`) was the dev server's HTTP library, `tiny_http`: its connection pool counts idle threads without a lock, so when a browser opened several connections at once one of them could wait for as long as another stayed open. The dev server has its own small HTTP server now (`crates/dev-server/src/http.rs`, one request per connection). 180 runs of the three tests that failed most, with 8 workers, all pass.
- Never build another checkout (e.g. a `git worktree`) into this repository's `target/`: Cargo reuses artifacts by crate id, so it silently hands the e2e tests a stale `notatnik-dev-server` (missing new fields). Use `CARGO_TARGET_DIR` for other checkouts.

- Fixed 2026-10-05: the "selecting blocks: indent all and copy as Markdown" flake (~1 in 10) was a real race. Selection commands (indent, outdent, move, delete) computed their ops from the page as it was when the key was pressed, so a Delete right after a Tab used the page from before the indent. They now build the ops when the request reaches the front of the page's queue (`applyWith`), like the editor commits. 30/30 runs pass.
- Phase 2 is implemented and browser-tested but not finished: WebKitGTK performance (S1), an AppImage, the `tauri-driver` end-to-end
  harness, paste/flush-on-quit browser tests and the manual checklist are open. See the status table in `docs/phases/phase-2.md`.
  UI tests run against the real Rust backend through `notatnik-dev-server` (decision D21).
- Page zoom (Ctrl +/-/0) is unit-tested only; check it once in a real `cargo tauri dev` window (needs the capability rebuild).
