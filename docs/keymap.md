# Keymap

The command table in `ui/src/lib/commands/table.ts` is the single source: the command palette lists it, and
this page mirrors it. Editor keys are handled in `Editor.svelte`, selection keys in `App.svelte`.

## Editing a block

| Keys | Action |
|---|---|
| Enter | Split the block at the cursor (new sibling; first child if the block has expanded children). Newline inside a code fence or below the first line |
| Shift+Enter | Newline inside the block |
| Tab / Shift+Tab | Indent / outdent the block with its children |
| Alt+↑ / Alt+↓ | Move the block among its siblings |
| ↑ on the first line / ↓ on the last line | Move the editor to the previous / next visible block |
| Backspace at the start | Merge into the previous block (an empty block is deleted; a block with children is not merged) |
| Delete at the end | Pull the next block into this one |
| Esc | Leave editing: the block becomes selected |
| Ctrl+Z / Ctrl+Shift+Z (Ctrl+Y) | Undo / redo typing in the block; when that history is empty, the global undo/redo |
| Ctrl+Enter | Cycle task state (phase 3) |
| `[[`, `((`, `#`, `/` | Autocomplete: pages, blocks, tags, slash commands |

## Selected blocks

| Keys | Action |
|---|---|
| ↑ / ↓ | Move the selection |
| Shift+↑ / Shift+↓ | Extend the selection (siblings only) |
| Enter | Edit the selected block |
| Tab / Shift+Tab | Indent / outdent the selection |
| Alt+↑ / Alt+↓ | Move the block |
| Delete / Backspace | Delete (one undo step) |
| Ctrl+C / Ctrl+X | Copy / cut as Markdown |
| Esc | Clear the selection |

## Anywhere

| Keys | Action |
|---|---|
| Ctrl+K | Command palette |
| Ctrl+Shift+F or `/` | Search |
| Ctrl+Z / Ctrl+Shift+Z | Global undo / redo (when not typing) |
| Shift+click a link | Open the page in the right sidebar |
