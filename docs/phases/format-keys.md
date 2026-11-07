# Formatting keys in the editor (FK1 to FK3)

**Goal:** the usual shortcuts for **bold**, _italic_, ~~strikethrough~~, ==highlight== and `code` in the block editor,
without losing the Emacs keys the editor already has. They write plain inline Markdown; the file format does not change.

**Requirements covered:** FK-1 to FK-7 (see `requirements.md`); builds on ED-2/ED-3 (the CodeMirror editor and its keys).
**Depends on:** `Editor.svelte` (the keymap), the edit path of the editor (one commit, `PageDoc.applyWith`).
**Status:** FK1 to FK3 done and tested (Chromium and WebKit). Open: the Mac's Cmd keys and the keys on the real desktop app.

## Decisions

- **A key that is also an Emacs key formats only when text is selected.** Ctrl+B is "cursor one character left" today
  (`keymap.md`). With a selection it wraps the selection in bold; with none it moves the cursor, as now. No setting.
  The other formatting keys have no Emacs meaning and always act on a selection.
- **Only a selection is formatted.** With nothing selected the formatting keys do nothing (Ctrl+B moves the cursor).
  You select a word first (double-click, Shift+arrows, Ctrl+Shift+A). There is no "bold mode" for typing.
- **Keys** (Cmd for Ctrl on a Mac, where Ctrl stays Emacs):

  | Action        | Keys         | Markdown     |
  | ------------- | ------------ | ------------ |
  | Bold          | Ctrl+B       | `**text**`   |
  | Italic        | Ctrl+I       | `*text*`     |
  | Strikethrough | Ctrl+Shift+X | `~~text~~`   |
  | Highlight     | Ctrl+Shift+H | `==text==`   |
  | Code          | Ctrl+Shift+C | `` `text` `` |

  Code is Ctrl+Shift+C and not Ctrl+E, so "end of line" with a selection left over never turns text into code. (Ctrl+Shift+C
  copies block references only in block-selection mode, not while editing text.) On a Mac Cmd+B and Cmd+I act **with or
  without** a selection: without one they do nothing (same rule), Cmd+B never moves the cursor.

- **Toggle.** The same key on text that is already wrapped in the marker (the markers inside the selection, or just
  outside it) removes them. Italic does not mistake the `**` of bold for its own marker: `**a**` is bold, not italic;
  `***a***` is both. Bold and italic combine by wrapping again.
- **The selection stays** around the text without the markers' characters (so a second style can follow), the whole change is
  **one undo step**, and trailing spaces of the selection stay outside the markers (`**word** `, not `**word **`).
- **Not in code.** The keys do nothing inside a fenced code block, and inside an inline code span only Code toggles.
- **Phone and tablet without keyboard:** no formatting buttons in this phase.

## Tasks

### FK1 The pure function

- `toggleWrap(text, from, to, marker)` in `ui/src/lib` (no editor, no DOM): returns the new text and the new selection,
  or null when nothing should change.
- **Verify:**
  - [x] unit tests: wrap a word, a phrase, a selection over several words; unwrap the same; markers just outside the
        selection and just inside it; `**a**` + italic, `*a*` + bold, `***a***` unwrapping each; a selection with trailing
        or leading spaces; an empty selection returns null; a selection over a `[[link]]` or a `((ref))` is wrapped whole;
        a selection spanning two lines (Shift+Enter) wraps each line; strike, highlight and code
  - [x] the returned selection is the text without the markers, in the new text

### FK2 The keys

- Bindings in `Editor.svelte`; Ctrl+B runs the formatting only when the selection is not empty and otherwise falls
  through to `cursorCharLeft`; the fence rule; one transaction for the change.
- **Verify:**
  - [x] with a selection Ctrl+B / Ctrl+I / Ctrl+Shift+X / Ctrl+Shift+H / Ctrl+Shift+C write the markers in the block and,
        after leaving it, in the file, in one Ctrl+Z step each; pressing the key again removes them
  - [x] without a selection Ctrl+B moves the cursor one character left; Ctrl+F, Ctrl+E and the other Emacs keys are as before;
        Ctrl+I, Ctrl+Shift+X and the others change nothing
  - [x] the selection is still there after the change, and a second key adds a second style (`***x***`)
  - [x] inside a fenced code block the keys change nothing
  - [x] the keys are consumed by the editor (no browser action); a link or reference in the selection is wrapped whole (unit test)
  - [ ] Cmd+B and Cmd+I on a Mac format a selection and never move the cursor (bound with `mac:`; the Linux browsers cannot run it)

### FK3 Tests and documents

- Playwright (Chromium and WebKit); `keymap.md` (the keys, the Ctrl+B rule next to the Emacs keys),
  `requirements.md` (FK rows), `testing.md`, `TODO.md`, `CLAUDE.md` (the doc list).
- **Verify:**
  - [x] the whole path per style: select a word in a block, key, leave the block, the file text, the rendered block shows
        bold/italic/strike/highlight/code, undo
  - [ ] `scripts/check.sh` and `scripts/check-webkit.sh` pass
  - [ ] by hand, in the desktop app (WebKitGTK): the keys with the real keyboard, also a non-US layout (Ctrl+Shift+X and
        Ctrl+Shift+H by their letters)
