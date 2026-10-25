# Two graphs at once (M1 to M5)

**Goal:** open two graphs side by side in one window, move or copy pages and blocks (with their images) from one to the
other, and always see which graph a note belongs to. Each graph keeps its own key, encryption rules, sync, undo and
settings.

**Requirements covered:** MG-1 to MG-6 (see `requirements.md`).
**Depends on:** nothing new; builds on the single-graph `AppState`.
**Status:** M1 to M4 built and tested. M5 (the second graph as a frame pane on the right) was built and reverted: it was not what was wanted. A second _sidebar_ panel for the other graph is being redesigned (see the end of this file).

## Design

- **A slot is one open graph.** `AppState` stays what it is: one graph with its index, watcher, sync loop, key and undo
  stack. The first state (the _root_) owns a registry of further states, one per extra _slot_ (`"1"`, `"2"`, ...; the
  root itself is slot `""`). Every command can name a slot; the root routes it. Nothing inside a state knows about other
  states, so keys, sync and undo cannot get mixed up.
- **Commands that involve two graphs** (`transfer_*`, `slots`, `close_slot`) are methods of the root. They take the
  slots of both sides.
- **Events carry the slot** (`graph-changed` gets `slot`; the root's events have none, which means the root).
- **The same graph cannot be open twice** in one app (two sync loops and two index writers on one folder): the second
  open is refused with `already_open`. Across two processes there is still no lock (as before).
- **A second graph does not become the start graph.** Only the root's `open_graph` writes `last_graph`.
- **Encryption does not get in the way of a transfer.** Files on disk are plain in every graph; each graph encrypts
  what it uploads by its own rules. A transfer only _warns_: a page that is encrypted in the source and would be plain
  on the target's server, and a target whose key is locked (its sync waits, nothing leaks).
- **The side pane is the same app in a frame.** The UI keeps its state in modules (recent pages, favorites, icons, sync,
  the open page ...). Keying every one of them by graph would touch the whole UI; instead the second graph is the same
  UI loaded in an `<iframe>` of the same origin with `?slot=1`, so each pane has its own stores, shortcuts and palette
  by construction. The frame has no connection of its own to the backend: it asks the parent window, which adds the
  slot to every call and forwards the events of that slot.
- **Identity.** Each graph has a color from a fixed palette (a device setting by graph id; default by a hash of the id)
  and always its name beside it: a bar on top of the sidebar, a dot in the graph switcher, the window title, a chip in
  the head of a page when two graphs are open, and "From X to Y" in the transfer dialog. Nothing is written into
  notes.

## Transfer rules

- **What:** a page, a page with its subpages, or a selection of blocks (with their children) into a page of the target.
- **Images and attachments** below `assets/` that the moved text mentions are copied to the same path. A file of that
  name with the same bytes is reused; with other bytes the copy gets a new name (`-<hash8>`) and the text is rewritten.
  The relative `../` prefix is recomputed for the target page's folder depth. Linked server files stay links.
- **Name clash:** keep both (the copy is named `Name (from Graph)`), append the blocks to the existing page, or skip.
  A journal day is always appended to the same day of the target.
- **Block IDs:** an ID that the target already uses gets a new one in the copy, and `((refs))` inside the copy follow
  it. References to blocks that did not move are listed.
- **Links** to pages the target does not have are listed (they become empty pages there). A move also lists pages of the
  source that still link to what moved.
- **Move = copy, then delete in the source** (to the source's trash, 30 days). _Undo_ removes the target's copies
  (to the target's trash, or restores the text an append changed) and brings the source back.
- **Tasks with dates:** the calendar item of a task is named after graph id and block id, so a copy is a new item in
  the target; the source's item goes away when the source's own sync sees the task gone (move).

## Tasks

### M1 Several graphs in one app (MG-1)

- [x] `AppState` owns extra states by slot; `dispatch_slot(state, slot, name, args)` routes; `slots` and `close_slot` commands
- [x] Events from an extra slot carry `slot`; a second state does not register as `last_graph`; `already_open` for a graph that is open in another slot
- [x] The backup password is shared by all slots; a last sync and snapshot on quit for every slot
- [x] Dev server (`?slot=` on `/api/<name>` and `/asset`), Tauri (`slot` argument, `asset://localhost/<path>?slot=`), `backend` in the UI knows its slot
- [x] Bindings regenerated

**Verify**

- [x] App test: two graphs open in two slots; a page written in one does not exist in the other; undo is separate; the same graph twice is refused; closing a slot leaves the other; the root's events have no `slot`, the other's have
- [x] Dev-server routing through HTTP with `?slot=`: `ui/tests/beside.spec.ts` (`Backend.beside()` opens the graph in slot 1 that way)

### M2 Transfer engine (MG-2, MG-3, MG-4)

- [x] `transfer_plan` (what would happen: pages, clashes, assets, missing links, encryption warnings, locked target) and `transfer_apply` (copy or move; pages with subpages, or blocks) with `transfer_undo`
- [x] Assets copied with the rewrite rules above; block IDs; journal days appended
- [x] Target not open in a slot: written through a temporary handle

**Verify**

- [x] Rust tests: page copy, subpages, clash (keep both, append, skip), journal append, image reused and renamed on different bytes, `../` depth, block IDs and refs, move trashes the source, undo of copy, of move and of append, encrypted source to plain target warns, locked target still writes, graph not open in a slot

### M3 Transfer in the UI (MG-2, MG-4)

- [x] _Copy to graph…_ and _Move to graph…_ in the page menu, the palette and the block menu; dialog with the plan, the choice for a clash and the warnings; an Undo toast
- [x] Pictures travel with the notes in the same call (no separate picture command: copying a picture is copying the block or page that shows it)

**Verify**

- [x] Browser tests: page copy and move from the dialog, warnings shown, undo, blocks into a new and an existing page

### M4 Which graph is this (MG-5)

- [x] `graph_color` device setting by graph id (palette index; default from the id); graph switcher dot and menu to change it; sidebar bar; window title; chip in the page head when two graphs are open

**Verify**

- [x] Unit test for the default color; browser test: computed color of the bar and chip, title text, never color alone (the name is there)

### M5 A second graph in the sidebar (MG-6)

**Design.** The sidebar gets one *section* per open graph, stacked: the header (color dot, name, padlock, the graph menu), then Journals, Agenda, All pages, Search, Settings, the sync line, Favorites, Recent, Saved queries and Pages of that graph. The first section is the graph in slot `""`, the second the graph in slot `"1"`. The second starts folded to its header; a click on anything in a section opens it. The page area shows what was opened last, from either graph, and the *active* graph is the one whose page it shows. Every UI store that holds the state of one graph moves into a `GraphState` (one per section): its backend, bus, recent pages, favorites, icons, archived pages, encryption status, sync status, calendar sync, saved queries and tree folds. No frames: each sidebar section reads its own `GraphState` through Svelte context, and the code of the page area keeps importing the same modules, which now answer for the active graph (thin facades over `active().…`).

- **Route.** `#/page/X` is a page of the active graph; `#/g/1/page/X` is a page of the graph in slot 1 (the prefix is on every route kind; slot `""` has none, so existing links and tests keep working). A hash with another slot makes that graph active; the page area is rebuilt (`{#key}`), the open documents are flushed and dropped, and editing state is reset. A `PageDoc` keeps the backend it was made with, so a late save never goes to the other graph.
- **Opening and closing.** *Open as second graph* in a section's graph menu (for the other graphs of this device); the second section's menu has *Close*. The second graph is remembered on this device and reopened at start.
- **Moving notes.** Drag a page row (tree, Recent, Favorites) or a block bullet onto a page row or the header of the other section, or onto the page area when it shows the other graph: the transfer dialog opens with source and target fixed. *To graph…* stays as the button.
- **Identity.** Each section has its graph's color along its header; the page area keeps the chip with the graph's name and color while two graphs are open; the active section is marked.

### M5 tasks
- [ ] R1 `GraphState` and the per-graph stores as classes; the old modules are facades over the active graph; the single-graph app behaves as before
- [ ] R2 Route with a slot, activation, a second graph opened and closed, remembered at start
- [ ] R3 The sidebar as sections (components read their graph from context)
- [ ] R4 Section colors, active mark, folding
- [ ] R5 Drag between sections and onto the page area; the dialog with source and target fixed
- [ ] R6 Docs, WebKit run, manual check list

**Verify**
- [ ] Unit tests for the route with a slot and for the per-graph stores (two instances do not share state)
- [ ] Every existing browser test passes unchanged (single graph)
- [ ] Browser tests: two sections with their own pages, Recent, Favorites, icons and padlocks; opening a page of the second graph switches the page area and back; edits go to the right graph (files on disk); closing the second section; a page row dragged to the other section's header opens the dialog with the target fixed and the copy lands; a block bullet dragged onto a row of the other section appends; a reload keeps both
- [ ] WebKit run of the new specs

## M5 as a frame, built and reverted

The first M5 showed the second graph as the whole app in a frame on the right of the page. It was reverted: it was not what was wanted. The backend slots, the transfer engine and dialog and the graph colors stay.

## Not in this phase

Links between graphs; the same graph twice; more than two graphs in the sidebar (the backend allows it); a lock file across processes.
