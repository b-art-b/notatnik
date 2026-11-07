# Numbered and lettered sub-bullets (LS1 to LS3)

**Goal:** choose, for one block, how its **direct children** are marked: the usual bullet dot (default), numbers
(`1.`), letters (`a.`) or roman numerals (`i.`). The choice is made from the bullet menu, is kept in the file, and
the marks follow the blocks when they are moved, indented or deleted.

**Requirements covered:** LS-1 to LS-6 (see `requirements.md`); builds on ED-1..ED-5 (blocks, collapse), the bullet menu
(`BlockMenu.svelte`) and the hidden-properties rule of `PropertyCard`.
**Depends on:** the block renderer (`Block.svelte`), the property parser (`crates/core/src/props.rs`), `PageDoc.applyWith`.
**Status:** LS1 to LS3 done and tested (Chromium and WebKit). Open: a look in the real desktop app.

## Decisions

- **Stored in the file, on the parent.** A block property `list:: numbered`, `list:: letters` or `list:: roman`. No
  property (or any other value) means bullets, so existing notes do not change. It is not in `state.json`: the style is
  part of how the note reads and must sync to the phone. Logseq's `logseq.order-list-type` is not read or written.
- **Direct children only.** The property on a block says how its own children are marked. Their children choose for
  themselves, and **restart** at `1.` / `a.` / `i.` under every parent (no `1.1`).
- **The mark is the face of the bullet, not a new element.** Same column, same guide lines, same drag handle, same
  click, menu and long press. Collapsed children are unaffected; a collapsed parent still shows its dot as now.
- **The number is the index among the siblings,** counted in the page order, so drag, indent, outdent, delete and undo
  renumber by themselves. Nothing is written to the children.
- **Letters and roman:** `a.`..`z.`, then `aa.`, `ab.` ...; roman `i.`, `ii.`, `iii.` ... lower case. Numbers over 999 and
  similar are shown as they are, not truncated.
- **Hidden, set from the menu.** `list` is a hidden property (not shown in the property card), changed in the bullet
  menu: _List style_ → _Bullets_ / _Numbered_ / _Letters_ / _Roman numerals_ (the current one ticked). _Bullets_ removes
  the property. It is also in the command palette for the focused block. The text of the block is never touched.
- **Copy keeps the property, it does not write `1.` lines.** _Copy with children_ and Ctrl+C of blocks put `- parent`,
  `list:: numbered` and the `-` children on the clipboard, because the same text is what a paste reads back as blocks
  (a `1.` line would paste as plain text, see `file-format.md`). A pasted block keeps its style. (Changed from the first
  plan, which wrote `1.` lines.)

## Tasks

### LS1 The property and the marks

- `list` joins the hidden properties; a pure helper `listMark(style, index)` in `ui/src/lib` (number, letters, roman);
  `Block.svelte` shows the mark instead of the dot for the children of a block with the property; the bullet column
  stays as wide for `iii.` / `aa.` as for the dot.
- **Verify:**
  - [x] unit tests for `listMark`: 1, 9, 10, 26, 27, 52, 4, 9, 14, 40, 90, 400, 3999 and a large number
  - [x] parse + no-op edit of a file with `list:: numbered` is byte-identical (golden file); `list` is not shown in the
        property card, and is listed by the property search as any property
  - [x] a parent with `list:: numbered` and three children shows `1.` `2.` `3.`; with `letters` `a.` `b.` `c.`; with `roman`
        `i.` `ii.` `iii.`; grandchildren keep dots unless their parent has the property, and restart at 1 under another parent
  - [x] drag, indent, outdent, delete and undo renumber; a collapsed parent and a zoomed block show the right marks
  - [ ] the children keep their guide lines, the drag handle on the mark, click to zoom and the long-press menu (the mark is
        inside the same button, so they work as before; the right-click menu is tested, the others not separately)

### LS2 The menu, the command and the export

- _Children as ..._ in the bullet menu (four items, the current one ticked; not for a read-only page or for text between
  bullets), four palette commands that apply to every selected block or the block being edited, the property written
  and removed through `PageDoc.applyWith` (`setPropertyOf`) so undo works; _Copy with children_ keeps the property.
- **Verify:**
  - [x] choosing _Numbered_ adds one line `list:: numbered` to the parent and changes nothing else in the file; choosing
        _Bullets_ removes it; Ctrl+Z restores each
  - [x] the current style is ticked in the menu; the entries are absent on a read-only page and in the menu of a
        multi-block selection; the palette commands apply to every selected block
  - [x] the palette entry works for the focused block, by keyboard only
  - [x] _Copy with children_ of a numbered parent keeps `list::` in the text, so a paste brings the style back
  - [ ] the style written by another program into the file shows after the file watcher's reload (not tested separately)

### LS3 Tests and documents

- Playwright (Chromium and WebKit); docs: `requirements.md` (LS rows), `file-format.md` (the `list::` property),
  `keymap.md` (menu and palette), `testing.md`, `TODO.md`, `CLAUDE.md` (doc list).
- **Verify:**
  - [x] the whole path for each style: set from the menu, reorder, undo, reload the page, the file text
  - [x] a screenshot check at 1850 px and 700 px: the marks do not shift the text column
  - [x] `scripts/check.sh` and `scripts/check-webkit.sh` pass
