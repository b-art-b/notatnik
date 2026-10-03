# Roadmap

Each phase has a detailed document with tasks, **Verify** steps for every task, exit criteria and a
manual acceptance checklist. The checklists live **only** in the phase documents; this file is the
overview. How verification works in general: [testing.md](testing.md).

Legend: ⬜ not started · 🟨 in progress · ✅ done

| Phase | Document | Goal | Status |
|---|---|---|---|
| 0 | [Environment, setup, risk spikes](phases/phase-0.md) | Toolchain, buildable empty workspace, check script; spikes on WebKitGTK performance, parser choice, CalDAV | 🟨 |
| 1 | [Core without UI](phases/phase-1.md) | File format frozen; parser, edit ops, query language, SQLite index, graph service (atomic writes, watcher), CLI, benchmarks | ⬜ |
| 2 | [Desktop app MVP](phases/phase-2.md) | Tauri + Svelte outliner: editing, keyboard model, undo, links, backlinks, namespaces, journals, search, packaging | ⬜ |
| 3 | [Tasks, queries, views](phases/phase-3.md) | Task states and dates, agenda, live query blocks, list/table/kanban/timeline views, repeating tasks | ⬜ |
| 4 | [Calendar (CalDAV)](phases/phase-4.md) | Two-way task sync and event display with Nextcloud, safe conflict/deletion handling | ⬜ |
| 5 | [V1 polish](phases/phase-5.md) | 3-way merge + conflict UI, multiple graphs, assets, aliases, unlinked refs, namespace moves, templates, drag-drop, smart paste, themes | ⬜ |
| 6 | [Plugins and scripting](phases/phase-6.md) | Sandboxed TypeScript plugin API with permissions, custom views, user scripts, example plugins, security suite | ⬜ |
| 7 | [AI (optional)](phases/phase-7.md) | Ollama provider, embeddings in sqlite-vec, hybrid search, related notes, chat with citations, privacy guarantees | ⬜ |
| 8 | [Android](phases/phase-8.md) | Tauri Android build, built-in WebDAV sync, mobile UI, quick capture, share target, CalDAV on mobile | ⬜ |

## Milestone summary

| After phase | You can… |
|---|---|
| 1 | Keep notes in a text editor and use the `notatnik` CLI to search, query, rename and check them safely |
| 2 | Use the desktop app daily for journals and notes |
| 3 | Manage tasks and project boards in the app |
| 4 | See and complete your tasks from the phone's calendar/tasks apps and Nextcloud |
| 5 | Use a complete V1 desktop app |
| 6 | Extend the app yourself |
| 7 | Search and ask questions by meaning |
| 8 | Use the app on Android |

## Later / ideas backlog

Not scheduled; may become phase 9+ or plugins.

- WASM plugins via Extism (PG-6)
- Graph view (LN-9)
- KaTeX and Mermaid (ED-12)
- Meeting notes from calendar events (CA-10)
- Email-to-note (IN-3), Nextcloud Deck/Notes (IN-4)
- Export / publish a static site from a namespace (IN-5)
- Windows / macOS builds (PL-2)
- Polish UI translation (NF-10)
- Background sync on Android via WorkManager
- Home-screen widget on Android
