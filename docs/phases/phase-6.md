# Phase 6: Plugins and scripting

**Goal:** a stable, sandboxed plugin system so new features (views, commands, integrations) can be
built without touching the core, plus lightweight user scripts for personal automation.

**Requirements covered:** PG-1–5, QV-9, IN-2 (example plugin).
**Depends on:** phases 2–3 (commands table, view interface), phase 5 recommended.

## Exit criteria

1. Every task checked, with every **Verify** item passing; `scripts/check.sh --full` passes, including the plugin security suite (P6.8).
2. Two example plugins are built using only the public API, with no core changes made just for them.
3. `docs/plugin-api.md` is complete enough that you can write a third plugin from the docs alone.

## Milestones

| | Milestone | Tasks |
|---|---|---|
| 6A | API design | P6.1–P6.2 |
| 6B | Runtime and sandbox | P6.3–P6.5 |
| 6C | Permissions and management | P6.6–P6.7 |
| 6D | Security verification | P6.8 |
| 6E | User scripts, examples, docs | P6.9–P6.11 |

## Key design decisions for this phase

| Topic | Decision |
|---|---|
| Execution | Plugin logic runs in a dedicated **Web Worker** per plugin (no DOM, no Tauri access). Plugin UI (views, panels, settings) runs in a **sandboxed iframe** (`sandbox="allow-scripts"`, no `allow-same-origin`), served from a custom protocol with a strict CSP. |
| Communication | Typed RPC over `postMessage` between the worker/iframe and the plugin host in the main UI. The host checks permissions and calls the app's API. Plugins **never** get Tauri IPC directly. |
| API shape | Versioned (`apiVersion` in the manifest; semver). Model-level only: pages, blocks, properties, queries, ops. No raw file access in v1 (answers the open question in requirements §18; revisit later). |
| Writes | All plugin writes are `BlockOp`s through the guarded path, grouped into one undo step per plugin call, and attributed in a log ("changed by plugin X"). |
| Distribution | Local only: plugin folders in `.notatnik/plugins/<id>/` (synced). No marketplace in v1. |

---

## 6A: API design

### P6.1 Plugin API specification
- [ ] Write `docs/plugin-api.md` *before* implementing: manifest schema, lifecycle (`activate`, `deactivate`), and API namespaces:
  - `app`: version, current page, navigate, notifications
  - `graph`: `getPage`, `getBlock`, `query`, `search`, `backlinks` (read); `applyOps` (write)
  - `commands`: register commands (appear in the palette and keymap)
  - `slash`: register slash commands
  - `views`: register view types for query blocks (QV-9)
  - `renderers`: register a renderer for a fenced code language (e.g. ```` ```chart ````)
  - `ui`: sidebar panels, settings page
  - `events`: `blockChanged`, `pageOpened`, `taskCompleted`, `graphChanged`
  - `storage`: per-plugin key-value storage (device-local) and plugin-data files in the graph (synced)
  - `net`: `fetch` restricted to allowed domains
- [ ] Each method lists the permission it needs and its error cases

**Verify**
- [ ] Review: every requirement PG-1 item maps to an API method
- [ ] Mock exercise: write the two example plugins (P6.10) in pseudo-code against the spec; no missing API is found (or the spec is updated)

### P6.2 `@notatnik/plugin-api` package
- [ ] TypeScript types + a thin client library (RPC wrappers) in `packages/plugin-api/`; generated from a single schema so the host and the client can't drift apart
- [ ] A template plugin project (`pnpm create`-style or a folder to copy) with build setup (Vite library mode → single `main.js` + optional `ui.html`)

**Verify**
- [ ] The template builds to a valid plugin folder; installing it shows "Hello from plugin" in the palette
- [ ] Changing a method signature in the schema causes a type error in both the host and the template (shared types)

## 6B: Runtime and sandbox

### P6.3 Plugin host and worker runtime
- [ ] Load enabled plugins on startup; one Worker each; RPC with request IDs, timeouts (default 5 s per call) and cancellation
- [ ] Lifecycle: activate on load, deactivate on disable/uninstall (with cleanup of registered commands/views)

**Verify**
- [ ] Enable → the command appears; disable → it disappears without a restart
- [ ] A plugin with `while(true){}` in `activate` → terminated after the timeout, an error is shown, and the app stays responsive (UI interactions still < 100 ms)
- [ ] A plugin throwing in a handler → error logged with the plugin name; other plugins unaffected

### P6.4 Sandboxed UI iframes
- [ ] Iframe sandbox + CSP (`default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:`); theme CSS variables injected so plugin UIs match the app
- [ ] Resize protocol (iframe reports its content height)

**Verify**
- [ ] A view plugin renders inside a query block, picks up the dark theme, and resizes with its content
- [ ] Security cases are covered in P6.8

### P6.5 View and renderer registration (QV-9)
- [ ] Plugin views receive `{rows, columns, groups, options}` like built-in views; edits come back as `BlockOp`s
- [ ] Fenced-code renderers receive the code text and return UI in an iframe

**Verify**
- [ ] `view: <plugin-view-id>` in a query block renders the plugin view; editing in it changes the file through the guarded path
- [ ] An unknown view ID → an inline error, not a crash

## 6C: Permissions and management

### P6.6 Permission model (PG-3)
- [ ] Manifest declares permissions: `graph:read`, `graph:write`, `net:<domain>`, `calendar:read`, `ai`; install/enable dialog lists them in plain language; changes to permissions on update require re-approval
- [ ] Optional scope: `graph:read:ns:<namespace>` (plugin limited to that namespace and everything below it, same meaning as `ns:` in queries)

**Verify**
- [ ] A plugin without `graph:write` calling `applyOps` → a permission error, no change
- [ ] A plugin scoped to `ns:reading` querying all tasks → only results from `reading` and pages below it
- [ ] Updating a plugin that adds `net:example.com` → the approval dialog appears again; declining keeps the old version enabled

### P6.7 Plugin management UI (PG-4)
- [ ] Settings → Plugins: list, enable/disable **per device** (state in device settings), install from folder/zip, uninstall (moves to trash), view permissions and logs

**Verify**
- [ ] A plugin installed on one device appears on another after sync, **disabled** until approved there
- [ ] Uninstall → the folder moves to `.notatnik/trash/`; the plugin's registered items disappear

## 6D: Security verification

### P6.8 Plugin security test suite
A deliberately malicious test plugin (`fixtures/plugins/evil/`) tries each attack below. All must fail, and each is an automated test (Playwright in the real Tauri build via `tauri-driver`, since the sandbox behaviour depends on the real webview):

- [ ] Access `window.__TAURI__`, `window.__TAURI_INTERNALS__`, or `window.parent`/`top` from the worker and from the iframe
- [ ] Call `invoke` / post forged messages pretending to be the host or another plugin
- [ ] `fetch` to a domain not in its permissions; WebSocket; `<img src=http://…>` beacon from the iframe (CSP)
- [ ] Read another plugin's storage
- [ ] Navigate the top window; open new windows; `alert`/`prompt` spam
- [ ] Write via `applyOps` without permission; write outside its namespace scope
- [ ] Exhaust memory (allocate in a loop) → worker terminated, app survives

**Verify**
- [ ] Every attack above has a test, and every test passes (the attack is blocked)
- [ ] Manual review of Tauri capabilities: the iframe/worker origins have no IPC permissions (record the capability config in `docs/plugin-api.md` §Security)

## 6E: User scripts, examples, docs

### P6.9 User scripts (PG-5)
- [ ] Scripts are single `.js` files in `.notatnik/scripts/` with a header comment declaring a trigger (command, slash command, or event) and permissions; they run in the same worker sandbox with an implicit manifest
- [ ] A built-in script editor (CodeMirror, JS mode) with a "Run" button and console output

**Verify**
- [ ] A script "move all DONE tasks from today's journal to an `archive` page" runs from the palette, is one undo step, and gives the expected file changes
- [ ] A script triggered on `taskCompleted` runs exactly once per completion

### P6.10 Example plugins
- [ ] **Habit tracker view**: a query view showing a grid of days × habits from blocks with `habit::` properties in journals
- [ ] **RSS import** (IN-2): a command that fetches configured feeds (`net:` permission per feed domain) and appends new items to `inbox/rss` as blocks, deduplicated by GUID

**Verify**
- [ ] Both plugins work using only `@notatnik/plugin-api`, with no core changes made just for them
- [ ] RSS import run twice → no duplicate items

### P6.11 Developer documentation
- [ ] `docs/plugin-api.md` completed: tutorial (build the template plugin), API reference, permission reference, security model, versioning policy, debugging (worker console, logs)

**Verify**
- [ ] You write a small third plugin (e.g. a "word count" command) following only the docs; note any missing explanations and fix the docs

---

## Manual acceptance checklist

- [ ] Install both example plugins; use the habit tracker for a week
- [ ] Write two personal user scripts you actually want
- [ ] Disable a plugin on the desktop, check it's still enabled on another device (per-device state)

Record: date / commit / notes:
