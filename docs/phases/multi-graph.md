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

### M5 Side pane as a frame (MG-6): built, then reverted

- [ ] _Beside_ next to each other graph in the graph switcher (the pane is there as long as the backend has a graph in slot 1, so a reload keeps it); the second graph in a frame on the right with a header (color, name, swap, close); the frame's backend goes through the parent; the pane's own switcher chooses another graph; drag a page or block from one pane to the other opens the transfer dialog

**Verify**

- [x] Browser tests: open beside, edit in both panes, each pane's recent list and encryption mark are its own, a transfer by drag, close the pane

## Not in this phase

Links between graphs; the same graph in two panes; more than two panes in the UI (the backend allows it); a lock
file across processes; remembering the side pane after a restart.

## What was decided on the way

- The second pane is a frame of the same app, not keyed stores (see Design). Its sidebar starts closed and that state is not remembered, so the first pane's setting is left alone.
- A drop of a block or page from the other pane never reaches the code that moves blocks inside a page: that code works from the module state of its own pane (`dnd.from`), which the other pane's drag does not set.
- The drag effect of a block is `move` and of a page `copyMove`; the drop target answers with an effect the drag allows, otherwise the browser cancels the drop.
- The transfer undo is one level and lives in the root state (`last_transfer`).

## M5 reverted

The frame pane (the whole app for the second graph on the right of the page) is gone. The backend slots, the transfer engine and dialog and the graph colors stay: a second sidebar for the other graph needs them. MG-6 is open again.
