# Notatnik

A local-first, block-based outliner for personal knowledge management, inspired by
[Logseq](https://docs.logseq.com/) but with its own file format, architecture and features.

- Plain Markdown files are the source of truth, synced via Nextcloud.
- Outliner editing with links, backlinks, block references and namespaces.
- Daily journals, tasks, live queries and custom views (list / table / kanban / timeline).
- Two-way CalDAV sync with Nextcloud Calendar.
- Plugins (TypeScript, later WASM) and optional local AI (semantic search, chat with notes).
- Linux desktop first, Android later.

## Documentation

| Document | Contents |
|---|---|
| [docs/requirements.md](docs/requirements.md) | Functional and non-functional requirements, with priorities |
| [docs/architecture.md](docs/architecture.md) | Tech stack, components, data flow, key design decisions |
| [docs/file-format.md](docs/file-format.md) | On-disk graph layout and Markdown dialect (draft spec) |
| [docs/testing.md](docs/testing.md) | Testing strategy: test levels, fixtures, mock backend, check script |
| [docs/TODO.md](docs/TODO.md) | Roadmap: phase overview and status |
| [docs/phases/](docs/phases/) | One document per phase: tasks, verification steps, exit criteria |

## Tech stack (summary)

Tauri 2 (Rust) · Svelte 5 + TypeScript · CodeMirror 6 · SQLite (FTS5, sqlite-vec) ·
custom Rust Markdown parser (native + WASM) · CalDAV (`libdav`, `icalendar`).

See [docs/architecture.md](docs/architecture.md) for rationale.
