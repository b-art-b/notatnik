# Keymap

The command table in `ui/src/lib/commands/table.ts` is the single source: the command palette lists it, and
this page mirrors it. Editor keys are handled in `Editor.svelte`, selection keys in `App.svelte`.

## Editing a block

| Keys | Action |
|---|---|
| Enter | Split the block at the cursor (new sibling; first child if the block has expanded children). Newline inside a code fence or below the first line |
| Shift+Enter | Newline inside the block |
| Tab / Shift+Tab, or Alt+→ / Alt+← | Indent / outdent (demote / promote) the block with its children |
| Alt+↑ / Alt+↓ | Move the block (with its children) up / down among its siblings, as org-mode's M-↑ / M-↓. Every press counts, also with the key held down; the cursor stays in the block |
| ↑ on the first line / ↓ on the last line | Move the editor to the previous / next visible block |
| ← at the very start / → at the very end | Move to the end of the previous / the start of the next visible block |
| Home / End | Start / end of the whole line, also when the text wraps over several rows on screen (Shift selects) |
| Backspace at the start | Merge into the previous block (an empty block is deleted; a block with children is not merged) |
| Delete at the end | Pull the next block into this one |
| Esc | Leave editing: the block becomes selected |
| Ctrl+Z / Ctrl+Shift+Z (Ctrl+Y) | Undo / redo typing in the block; when that history is empty, the global undo/redo |
| Ctrl+Enter | Cycle task state (phase 3) |
| `[[`, `((`, `#`, `/` | Autocomplete: pages, blocks, tags, slash commands |

## Selected blocks

Select blocks with **Esc** in a block (it selects that block), with the mouse (below) or with **Ctrl+A**. A selected
block takes its children along: copy, cut, duplicate, delete and the other commands work on the *top* selected blocks
and everything below them. Every command is one undo step.

| Keys | Action |
|---|---|
| ↑ / ↓ | Move the selection |
| Shift+↑ / Shift+↓ | Extend the selection from where it started, over every block between as they appear on screen (children included; collapsed children come along) |
| Ctrl+A | Select all blocks of the page (in a block: first the text of the block, a second Ctrl+A all blocks) |
| Enter | Edit the selected block |
| Tab, or Alt+→ | Indent the selection (blocks at one level) |
| Shift+Tab, or Alt+← | Outdent the selection, whatever the levels: each block moves up one level, top-level blocks stay |
| Alt+↑ / Alt+↓ | Move the selected blocks together (blocks at one level) |
| Delete / Backspace | Delete |
| Ctrl+C | Copy as Markdown (indented outline), without block IDs |
| Ctrl+X | Cut: copy with the block IDs, then delete. Pasted again the blocks keep their IDs, so references to them survive |
| Ctrl+V | Paste the copied blocks right below the last selected block, at its level, with their structure; the pasted blocks are then selected |
| Ctrl+D | Duplicate: a copy of the selected blocks (with children, without IDs) right below the last one; the copies are selected |
| Ctrl+Shift+C | Copy a `((id))` reference per block, one per line (blocks without an ID get one) |
| Ctrl+Enter | Cycle the task state of every selected block: all get the state that follows the first one's |
| Esc | Clear the selection |

**Mouse.** Marking text inside one block with the mouse (to copy it with Ctrl+C) keeps it marked: releasing the button does not open the editor; a plain click edits. **Shift+click** a block selects everything from the block being edited or selected to the one clicked, as on screen. **Ctrl+click** adds a block (with its children) to the selection or removes it again. **Dragging** from the text of one block over other blocks selects them all; a drag inside one block stays an ordinary text selection. **Right-click** a selected block (with several selected) for a menu: Copy, Cut, Duplicate, Copy block references, Save as template…, Delete.

Pasting several lines while editing a block (not selecting) still makes several blocks, as before.

## Anywhere

| Keys | Action |
|---|---|
| Ctrl+K | Command palette: pages, commands, and **Create page** for a name that matches no page (Ctrl+Enter creates the page named as typed) |
| Ctrl+Shift+F or `/` | Search |
| Ctrl+Z / Ctrl+Shift+Z | Global undo / redo (when not typing) |
| Shift+click a link | Open the page in the right sidebar |

**New page** is part of the command palette (**Ctrl+K**, which also closes it): type a name (`people/Anna` makes a page in the namespace `people`). A name that matches no page shows **Create page “name”** as the last row (when a page or command matches, that one wins Enter; choose the create row with ↓ and Enter) or press **Ctrl+Enter** at any time to create the page named as typed. The page opens with its first block ready for typing; nothing is written to disk until you type something, so Esc or leaving it empty creates no file. An existing page with that name is listed and opened instead (there is no create row for it). A name the app cannot use (`../x`) is explained in the palette, which stays open.

**Search syntax** (Ctrl+Shift+F, `/`): words are combined with AND (`budget report`, or `budget AND report`, `budget & report`); `OR` or `|` means either (`forecast OR plan`); `-word`, `!word` or `NOT word` leaves out what has the word (`budget -draft`; `-(a OR b)` leaves out a group); `( )` groups (`(budget OR plan) report`); `"annual plan"` is a phrase (the words next to each other); `rep*` is a prefix. NOT binds tightest, then AND, then OR. `AND`, `OR` and `NOT` are operators only in capitals: `this or that` searches for all four words. Diacritics and case are ignored (`zolw` finds `Żółw`). Every search needs a word that must match: `-draft` alone or `a OR -b` is explained in the dialog (try `a -b`). The matching words are marked in the results; excluded words are not. A short reminder of the syntax is shown under the search field.

## View

| Key | Action |
| --- | --- |
| Ctrl + `+` / Ctrl + `=` | Zoom in (Firefox steps: 30 % … 500 %) |
| Ctrl + `-` | Zoom out |
| Ctrl + `0` | Reset zoom to 100 % |
| Ctrl + mouse wheel | Zoom in/out |

The zoom level is remembered per device (browser/webview storage), not in the graph.

## Side panel

A panel at the **right** can hold a page's **references** (linked and unlinked) and its **tasks** instead of the bottom of the page: *Settings → Appearance → References / Page tasks → Side panel* (each on its own; this device). It follows the page you are on and has **tabs** with live counts: *References*, *Tasks* (each only when it is set to the side) and *Pages* (the pages opened with Shift+click); ←/→ move between the tabs, the chosen tab is remembered, and with one tab there is no tab bar. It shows in windows at least about 1180 px wide; in a narrower window both go back below the page. **Ctrl+Shift+\\** (or the button at the panel's edge, or the palette) hides and shows it; drag its left edge to resize it: there is no upper limit. While the page keeps at least 320 px it narrows; dragged further (or → ←  with the edge focused, or the ⤢ button at the panel's edge, or a double click on the edge, or *Focus on the side panel* in the palette) the page is **folded into a strip** and the panel gets the whole window; the strip's *Show page* button, dragging back, → on the edge, or the same controls bring the page back (this mode is not remembered across restarts). The minimum width is 280 px; with the edge focused ←/→ move it by 10 px, Shift by 40, Home resets it to 380. Hidden, the references and tasks are not shown at all until you show it again.

## Sidebar

The button on the sidebar's edge (or **Ctrl+\\**, or "Show or hide the sidebar" in the palette) hides and shows the left sidebar; drag its right edge to resize it
(180 to 560 px; with the edge focused, ←/→ move it by 10 px, Shift by 40 px, Home or a double click resets to 240 px). Both are remembered on this device. The width can also be set in Settings → Appearance → *Sidebar width* (slider, Reset).

## Sync with a server

- Start screen → **New graph on a server…**: server, user name, app password, folder (type it, or **Browse…** once the login is filled in: click folders to go in, the path above to go back up, *↰ Up*, *New folder here* for one that does not exist yet, *Use “name”* to fill the field and check it; hidden folders are not listed). *Check the folder*, then *Create the graph* (new or empty folder) or *Download and open* (a folder that already holds a graph).
- A graph that is only a local folder: Settings → Sync → **Connect to a server…** (the same form). *Check the folder*, then *Upload this graph*: the pages stay where they are and go to an empty or new server folder; a folder with anything in it is refused.
  - Encryption can be set up and pages marked on a local graph, before connecting: **Upload this graph** then sends them encrypted from the first sync, so no plain copy ever reaches the server.
- Sidebar line `☁ Synced 3 min ago` (under the graph's save status): click to sync now; the tooltip says so. **Sync now** is also in the command palette (Ctrl+K). The line reads *Syncing…*, *Offline*, *Sign in needed*, *Sync problem* or *Stopped: many deletions* when something needs a look.
- Settings → Sync: the server folder, the last sync, Sync now, a password field when the login is refused, *Delete N files and sync* after a stop for many deletions, and *Disconnect…* (the notes stay on this device).
- A page changed on two devices is merged block by block when the changes do not overlap. Otherwise the server's version is kept and yours is saved next to it as a "conflicted copy".
- **Encryption** (EN-11 to EN-18; Settings → Sync → Encryption, and the *Encrypt…* button of a page): some files can be stored encrypted on the server while the rest stays as it is. On this device everything is plain Markdown: you edit, search, link and query encrypted pages like any other. File names, folders and sizes stay visible on the server; the text does not (an encrypted `pages/private/a.md` is stored as `pages/private/a.md.enc`).
  - *Set up…* asks for a password (8 to 200 characters, twice) and shows a **recovery key** once; tick "I saved the recovery key", then *Turn on encryption*. The vault is `.notatnik/vault.json` in the graph (public, it syncs); it is not the vault of the backups.
  - On any page, also on a graph that is not connected to a server yet, *Encrypt…* (next to *History*) stores the page encrypted; with *Also the pages below it* the folder of its subpages too. The dialog warns before it does anything: **the server already holds the text from earlier syncs** (its file versions and trash bin keep it; delete them in Nextcloud), **pictures** the page uses stay plain unless their folder is marked too, and **tasks** of the page will not be sent to the calendar. A 🔒 beside the title shows an encrypted page; *Encrypted* (the button) opens the dialog with *Stop encrypting*. Pages inside an encrypted folder can only be released by removing the folder (Settings).
  - **Key:** *Unlock…* (password or recovery key) keeps the key in memory until *Lock* or until the app closes. The switch *Keep the key in the system keyring* lets the app unlock itself when the graph opens (needs an unlocked key; anyone logged in to this computer can then use it). While the key is locked, plain files sync as usual and encrypted files are left alone on both sides: not sent, not received, not deleted. The sidebar says *🔒 N encrypted files waiting for the key*.
  - *Tasks of encrypted pages in the calendar* is off by default. Moving or renaming a page into or out of an encrypted folder is warned about in the rename dialog. Settings → Sync warns when backups are plain while pages are encrypted (Settings → Backup has its own encryption). A local file whose name ends in `.enc` is not synced (the ending is reserved) and is listed.
  - *Encrypt everything…* (EN-21, key unlocked) marks `pages/`, `journals/`, `assets/` and `templates/` at once, after a confirmation that shows the number of pages and the same warnings as a single page. `.notatnik/` (the vault and the settings) is never encrypted.
  - *Forgot the password…* (EN-22; also under Settings → Backup → Encryption) is for a password **and** recovery key that are lost: after a checkbox it makes a new vault and recovery key that replace the old ones. The files here are not touched; encrypted copies on the server are stored again with the new key at the next sync, and other devices need the new password. What exists encrypted only on the server or another device, and sealed backups made earlier, cannot be read any more.
  - The list of marked paths is `[encryption] paths` in `.notatnik/config.toml`; *Stop encrypting* in the list removes one.
- **A page another editor reformatted**: when a sync downloads a page in which a block ID is followed by a property on the same line (`- TODO call ^abc123 deadline:: 2026-10-21`; this app writes properties on lines of their own), the sidebar shows *⚠ 1 page(s) look reformatted* under the sync line, and Settings → Sync lists the files. It happens when the page is saved in Nextcloud's web *Text* editor, which joins the lines of a list item; the properties then stop working. Open the page, choose *History* and restore the version from before. The warning goes when the page looks right again.
- **Sync conflicts:** the warning above the page lists each conflicted copy; click one to open the resolver. It shows the blocks that both sides changed (before, the page, the copy) with *The page / The copy / Both / Edit…* per block (for a block deleted on one side: *Keep it / Delete it*), and the page as it would become, as a diff. *Write the merge* saves it and moves the copy to `.notatnik/trash/`; *Keep the page* and *Use the copy* skip the merge. *Settings → Sync → Merge changes from two devices* turns the automatic merge off.

## Images

- **Paste a link** (ED-27): an address pasted over selected text makes `[the text](address)`; a link copied from a web page (select its text, copy) keeps its text; tracking parameters (`utm_source=…`, `fbclid=…`) are removed; a `notatnik://page/Plan` address becomes `[[Plan]]` (over a selection `[[Plan|text]]`, with a block `((id))`); a Nextcloud Files address (`…/apps/files/?dir=…&fileid=123`) becomes `[Nextcloud file](…/f/123)`, which keeps working when the file is moved. Nothing is changed inside a link's address, a `[[ ]]`, `(( ))` or code, and an ordinary address with nothing to clean is pasted as it is. *Settings → Appearance → Look up titles of pasted links* (this device, off by default): a lone web address becomes `[page title](address)` once the site answers (the app contacts that site); Ctrl+Z gives the plain address back; leaving the block before the title arrives keeps the address.
- **Paste** a picture (a screenshot) while editing a block: the link `![](../assets/2026/10/pasted-1a2b3c4d.png)` is inserted at the cursor and the file is stored in the graph's `assets/` folder. A clipboard that also holds text pastes the text.
- In the desktop app a paste that brings no text and no file (the webview does not pass a screenshot from Flameshot or Spectacle on every system) makes the shell read the picture from the system clipboard instead.
- **Drop** an image file on a block that is being edited: the link goes where it is dropped, and the file name is kept in the stored name.
- The same picture twice is stored once. Only png, jpeg, gif and webp are accepted, up to 25 MB.
- Images are never deleted by the app on its own, not even when their block is. Settings → Graph → **Unused images** lists the files in `assets/` that no page mentions (a path anywhere in a page's text, in a link or in code, counts as a mention) and **Delete all** (or **Delete** on one image) moves them to this device's trash (outside the graph, so not synced). Each row has a preview (click to enlarge) and **Copy link**: pasting the copied `![](assets/...)` into a block writes the path that suits the page it lands on.

## Slash commands

Type `/` after a space, at the start of a line, or after punctuation (`(/todo`). With nothing typed the menu is grouped (Tasks, Dates, Task details, Blocks); as soon as you type it is
one list ranked by relevance (name, then keywords such as `/task` → To do, then descriptions). Each row shows an icon, the label, a one-line
description and the `/name`. Use ↑/↓ to move, Enter to choose, Esc to close.

**In the middle of a word** (`some wor/d`) the menu opens too, but offers only the commands that change the block's metadata (task marker, `/log`, `/scheduled`, `/deadline`, priorities), and only those whose name starts with what you typed: the command is removed from the word and its result goes where it belongs (marker at the start of the line, a log line in the logbook, a property line under the first line), never into the word. Commands that insert text (`/today`, `/date`, `/emoji`, `/code`, ...) need a space or punctuation before the slash. URLs, paths and fractions (`https://x.y/z`, `a/b`, `1/2`) and code fences are left alone.

`/todo`, `/doing`, `/waiting`, `/done` and `/canceled` can be typed anywhere in a block: the command is removed and the task marker is put at the start of the first line (replacing an existing marker).

`/later` and `/now` set the Logseq-style markers `LATER` (to do) and `NOW` (in progress); Ctrl+Enter cycles `LATER` → `NOW` → `DONE`. The agenda, page tasks and the board treat them like `TODO` and `DOING` (a board gets a *later* / *now* column when a task uses it); in queries write `task: todo, later` and `task: doing, now`.

Each task state has its own color on the marker pill: blue `TODO`, teal `LATER`, orange `DOING`, red-orange `NOW`, purple (ring) `WAITING`, green with a tick `DONE`, grey `CANCELED` (`--task-*` in `app.css`, light and dark).

The priority of a task shows right after its marker as `[#A]` (click it to change; none by default). The priorities are a graph setting: any number from one to nine, any labels (`A B C` by default, `1 2 3 4`, `!! !` ...): Settings → Tasks → *Priorities* (edit, + Add, − Remove last), or `[tasks] priorities = ["A", "B", "C"]` in `config.toml`. Changing them re-reads the files; a file that uses a label that no longer exists gets a warning and shows no priority. There is a `/` command for each: `/priority` (the most important), `/priority-b`, `/priority-c`, ... (the name follows the label: `/priority-p4`), and `/priority-none` to remove it. Each sets the `priority::` property line of the block (replacing an existing value). Like `/scheduled` and `/deadline` they can be typed anywhere in the block, also on a later line: the command is removed and the property goes directly under the first line, after the block's other properties. Task markers go to the start of the first line, `/log` into the `:LOGBOOK:` drawer, so a command always puts its result in its own place whatever the cursor position.

`/log` adds a dated line (`YYYY-MM-DD HH:MM `) to the block's logbook and leaves the cursor after it, ready for the note; the logbook is a `:LOGBOOK:` ... `:END:` drawer (file-format §3.6), shown in the rendered block as one more labelled row under `SCHEDULED:` / `DEADLINE:`: `LOGBOOK: <date> <latest note> +2 earlier`. Click the label (or "+n earlier") to open the whole history as a timeline, newest first; open logbooks are remembered on this device.

`/date`, `/scheduled` and `/deadline` open a date picker under the cursor: type a date (`tomorrow`, `next friday`, `+3d`, `2026-12-24`), click a quick choice, or click a day in the calendar (month arrows; week starts on the graph's `week_start`). Dates are always shown as `YYYY-MM-DD`: the calendar is our own, because the native date input follows the OS locale. Enter or a click applies, Escape cancels. For `/scheduled`, `/deadline` and the chips the picker has a **Time of day** switch, off by default (a plain date): switched on it shows a start time, an optional end time (24 h `HH:MM`, `9:30` is written `09:30`) and quick durations (30 min, 1 h, 2 h, *No end*); a day click, Enter or a quick choice then writes `2026-10-14 09:00-10:30` (or `2026-10-14 09:00` without an end). An end that is not after the start is refused with a message. Opened on a value with a time, the switch is on and the fields are filled; switching it off writes a plain date. The task chips (`⏳`, `📅`) open the same picker, prefilled, with a Clear button. `/date` inserts `[[YYYY-MM-DD]]` at the cursor (a link only; it does not make a due date). `/scheduled` and `/deadline` set the `scheduled::` / `deadline::` property line of the block, replacing an existing value; a new line goes directly under the first line, after the block's other properties and above a `:LOGBOOK:` drawer or body text (properties are only read there). `/today`, `/tomorrow` and `/yesterday` still insert a link directly.

## Path to the focused block

While a block is being edited, the path to it is drawn as in Logseq: a dark line leaves the bullet of each ancestor, runs down its guide line and turns
into an elbow that ends at the bullet of the next block on the path, down to the focused block. The guide line runs under the parent's bullet.

## Agenda

The buttons above the filters switch the view: **sections** (the classic agenda below), the same open tasks as **list**, **table**, **kanban** or **timeline**, the views of query blocks, or **calendar** (below). The filters (namespace, tag, priority), *Group* (page, state, deadline, scheduled) and *Sort* (date, deadline, scheduled, priority, page, state) apply to all of them; in the sections view they work inside each section (and each day): *Sort* orders the tasks ("date" keeps the agenda's own order), *Group* adds a heading with a count per page, state or date. The other views add *Show done*. *show query* displays the query that is run, and *save* puts it in the sidebar as a saved query. The choice is remembered on this device. In the sections view every task shows the note it comes from as a small chip at the right end of the task's own first row (the full path of parent blocks is in its tooltip); a click opens the note with the task flashed. In a narrow window the chip drops under the task. In the kanban view the board always includes done and canceled tasks (a card moved there stays on the board); drag a card to another column with the pointer, or use the ‹ › buttons on the card or Alt+← / Alt+→; a plain click opens the note with the task flashed. In the timeline, drag a task to another day (or Alt+← / Alt+→ for one day), a click opens the note.

**Calendar mode** (Agenda → *calendar*): *day*, *week* (the default) or *month*, with ‹ › and *Today*; the agenda's filters (namespace, tag, priority, *Show done*) apply to the tasks. Day and week are a 24-hour grid you scroll (it starts near the current time; a red line marks it), with a strip for all-day items on top. A task with `scheduled:: 2026-10-14 09:00-10:30` is a block of that length, with a start only an hour, a timed `deadline::` the hour that ends with it (dashed outline); a date without a time is in the strip. Items that overlap share the width. Tasks are outlined with a checkbox (tick it to complete the task); events of the calendars chosen in Settings are filled, read only, and open a small detail window. Click a task to open its note. The row above the grid has the **legend**: a switch and a colour for each calendar (the colour is kept in the graph, so it is the same on your other devices), *Tasks* and *Events* switches, and *Placed by* (the earlier of scheduled and deadline, or one of them) and *Overdue tasks*: **at their dates** (default) or **rolled over to today**. Rolled over puts every open task planned for an earlier day into today's strip, marked `↻ 3d` (three days late; hover for the day it was planned for), so the day shows what is still to do. Nothing is copied or changed in your files, finished tasks stay where they were done, and a week that does not include today shows the original dates. Drag a rolled task to give it a real new day. Tasks are never dropped by the calendar. **Drag** a task to another day or time (its length stays), drag its **bottom edge** to change the end, or use Alt+← / Alt+→ (a day) and Alt+↑ / Alt+↓ (15 minutes); dropped on the all-day strip it loses its time. The **No date** list on the right holds tasks without a date: drag one onto the grid to give it that day and an hour. **Click an empty slot**, type a title and press Enter to add a task of one hour to today's journal (Escape cancels). *Month* shows each day's items as chips (four, then *+n more*), drags tasks between days, and a click on a day number opens that day.

Sections: Overdue, Today, Next 7 days, **Later** (every open task dated further ahead, earliest first), No date.

Each task shows the note it lives in (`↗ page › parent`). Click it to open that note with the task flashed.

## Graphs: start screen and history

At start the app shows the **start screen**: the graphs you opened on this device, the most recent first. The last one is marked *last opened* and its **Open** button has the focus, so **Enter** opens it. A graph whose folder is gone is marked *folder not found* (it can only be removed). **✕** removes a graph from the list: only the list changes, the folder, its notes, its cache and its backups stay, and opening the folder again puts it back. Below is a field to open another folder (type or paste its path) or **Choose a folder…**, the system's own folder chooser (desktop app only), which opens the folder you pick, or makes it a graph when it has none. The same button is in Settings → Graph.

The **graph name at the top of the sidebar** opens a menu: the other graphs of this device (a click opens one), *Add new graph…* and *All graphs…* (both show the start screen). Esc or a click elsewhere closes it.

**Switch graph…** (command palette, or Settings → Graph → *Your graphs*) shows the same screen while a graph is open (*← Back to …* returns); opening another graph restarts the window on it. *Settings → Graph → Open the last graph at start* (this device, off by default) opens the last graph without showing the start screen. Only graphs opened in the app are listed: the command line tool does not add to the history.

## Settings

The list at the left has ten sections in four groups: **Look and feel** (*Appearance*, *Dates & language*), **Notes** (*Tasks*, *Page icons*, *Templates*), **Servers** (*Sync*, *Calendar*, *Linked files*) and **Data** (*Backup*, *Graph*). Arrow keys, Home and End move between them. The box above the list, *Search settings*, narrows the list to the sections that hold a setting you name ("encrypt", "zoom", "week"); Enter or the first match opens it, Escape clears it. Every section starts with its title, a pill that says where its values live, *This device* (`settings.toml`, never synced) or *This graph* (`.notatnik/config.toml`, synced), and one line about what the section is for. In a narrow window the list becomes a row you scroll sideways. Long explanations (how templates work) are folded away until you open them. Changes apply immediately and show "✓ Saved" at the top. Graph settings can also be edited by hand in `.notatnik/config.toml` (or arrive by sync): the open app notices within a second and applies them, no restart; a file that does not parse as TOML is ignored until it is fixed. Theme, week start and the date format are choice buttons, link brackets is a switch, zoom has − / + / Reset.

- **Language** and **Format** control how dates are shown in journal headings, the agenda and `[[date]]` links: `Sat, 3 Oct 2026`, `3 October 2026`, `03/10/2026`, `2026-10-03`, or a custom pattern such as `dddd, D MMMM YYYY` (see [date-formats.md](date-formats.md)). Date pickers, query clauses and files always use `YYYY-MM-DD`.
- **Show brackets around links** is off by default, so rendered links look like plain links; the raw `[[...]]` always appears while a block is edited.

## Mouse and history (like Firefox)

| Input | Action |
| --- | --- |
| Mouse back / forward button, Alt+← / Alt+→ | Back / forward in the page history. Alt+arrows only when no text field has the focus and no block is selected (in a block, and on selected blocks, they move blocks as above) |
| Middle click, Ctrl+click on a page link | Open the page in the right sidebar (Shift+click does too) |
| Click a day's title in the journals feed (also today's) | Open that day as a page |
| Ctrl+wheel, Ctrl +/-/0 | Zoom |

## Block menu: copy a reference

**Right-click a block's bullet** (or **Alt+click** it) for the block menu:

- **Copy block reference** puts `((abc123))` on the clipboard. Paste it into any block, on any page: it becomes a link to this block (shown with the block's text; the block lists the place in its references). The block gets its `^abc123` ID when it had none.
- **Copy as embed** puts `!((abc123))`: pasted, it shows the block (with its children) in place, editable.
- **Copy block text** and **Copy with children** (as Markdown, with indentation) copy the text.
- **Zoom in** is the same as clicking the bullet.

The top of the menu shows the block's ID (`^abc123`, or "no ID yet"), and the references in the menu show it too. A block that has an ID also shows it in its bullet's tooltip. IDs are otherwise hidden: they are in the file at the end of the block's first line, not in the editor.

A property whose value is a block reference (`follows:: ((abc123))`) is shown as `↳` and the text of that block, a link to it; its tooltip has the ID and the page. A reference to a block that no longer exists shows "broken ref".

↑/↓ move in the menu, Enter chooses, Esc or a click elsewhere closes it. It also opens on bullets in linked references and query results, where Zoom in is left out. The `((` autocomplete (type `((` and some words of the block) still inserts a reference without leaving the keyboard.

## Pages written by other editors (Nextcloud Text, Obsidian)

- Text between bullets (headings, tables, code blocks, footnotes after a blank line) is shown without a bullet, as it is in the file. Click it to edit it as raw Markdown: Enter adds a line, Tab and Backspace at the start do not nest or merge it, a paste stays as typed. It cannot have a task marker, properties or a block ID, and cannot be indented; use `+` at the end of the page or the bullets around it for outline blocks. Moving a bullet next to such text adds a blank line between them.
- A checkbox bullet (`* [ ] call`, `- [x] done`) is a task: shown as ☐ / ☑, clicking it toggles open and done and writes `[x]` / `[ ]` back, with no `done::` line. Marking it DOING (or another state with no checkbox) writes the word.
- Also drawn instead of shown raw: `::: info` ... `:::` callouts (a coloured bar; `info`, `warning`, `danger`, `success` and others), `<details><summary>Title</summary>...</details>` as a section that opens on a click on the title, footnote marks `[^1]` as a small raised number with the footnote text where it is written, and ```` ```mermaid ```` blocks as a diagram (drawn on first use; if the diagram has an error the code is shown with the reason). Clicking the text still edits the Markdown as written. Other HTML stays literal text.
- Images and files from `.attachments.N` folders next to a page are shown: a picture is displayed, any other file is a link that opens it with the system's program.

## Follow-up tasks and chains

`/followup` (also found by `/next`, `/done-next`, `/follow-up`) works on the task you are editing, from anywhere in the block: it marks the task **done** (completion time written, a repeating task advances, exactly as Ctrl+Enter) and puts a new **TODO** right after it, as the next sibling. The new task carries the tags, page links and priority of the old one (not dates, not the repeat), links back with `follows:: ((id))`, and the editor goes into it at the end of its first line, so you type what comes next. The finished task gets a block ID (`^abc123`) if it had none; nothing else of it changes. On a task that is already done only the follow-up is added; on a block that is not a task it becomes done and gets the follow-up.

A task that follows another, or has a follow-up, shows a **⛓ chain** button (a small ⛓ in the compact view). Click it for the whole chain from the first task to the last: state, text, the page and the date; the task you opened it from is bold; a click on a task opens it in its note. A task with several follow-ups is a branch (indented). A task whose predecessor was deleted shows "a task that was followed is gone". In queries `chain: ((abc123))` lists the whole chain of a task by its ID (combine with `task: todo` for what is still open). `follows::` can also be written by hand on any task.

## Times of tasks

`scheduled::` and `deadline::` may have a start and an end time on the day: `scheduled:: 2026-10-14 09:00-10:30`, `deadline:: 2026-10-14 17:00`. Set them in the date picker (*Time of day*). The chips show them (`⏳ 2026-10-14 09:00-10:30`), the compact indicator shows `today 09:00–10:30`. In the agenda a day's tasks are ordered by start time, those without a time last. In the timeline view the time is shown in front of the task (a marker; no length is drawn), and dragging a task to another day keeps its times. Overdue is still decided by the date, and queries (`scheduled: today`) match on the date. Repeating tasks keep the time and the length when they move to the next date.

## Compact task details

Settings → Appearance → *Task details* (this device): **Always shown** is the default. **Compact** replaces the lines under a task on pages and journals with small indicators at the end of its row: one date (📅 the deadline, else ⏳ the scheduled date; `3d overdue` in red, `today`, `tomorrow`, otherwise the date), `↻` for a repeating task and `✎ 3` for the logbook entries. Hover or focus them (Tab), or click one to keep the details open: the usual SCHEDULED / DEADLINE / REPEAT lines and the logbook appear under the row, and the date chips still open the date picker. Escape closes them. A finished task shows no date indicator. The agenda, a page's task list, query results and linked references always show everything. Custom properties are not part of this setting; see *Block properties*.

### Compact block properties

Settings → Appearance → *Block properties* (this device, independent of *Task details*): **Always shown** is the default; **Compact** replaces the property card under a block with a small `⋯ 2` at the end of its row (the number of visible properties; a task's scheduled, deadline, priority and repeat are not counted, and the tooltip lists `key: value`). Hover or focus it, or click to keep it open: the card appears under the row, and ✎ edit, × remove and + add work there. Escape closes it. The popover is as wide as its content (an open logbook is not squeezed into a column) and stays open while you type in one of its inputs. A block without properties has no indicator (add the first one with `::` or `/property`). The agenda, query results and linked references always show the card.

## Journal days as pages

A journal day is a page like any other: open it as its own page (click the date in the feed) and its `▸ properties` section works as on every page. A day has the default `type: journal` (shown muted; set a `type` there to replace it). Because of it, a page icon rule `type = journal` gives every day an icon: Settings → Page icons → *Add an icon for journal days*. Until an icon applies, the calendar glyph is shown. Today's day, which has no file until you write in it, gets the rule's icon too; emoji in titles are drawn a little smaller than the heading so they sit on the text line. A day's own `icon:` property wins, as everywhere.

## Block properties

A block's `key:: value` lines (right after its first line) show as a compact card under the block: bold key, then the value. Link values (`[[Acme]]`) open the page (no glyph, they are coloured as links; date, number and boolean values get a small one); another value opens the view of all blocks with that key and value. Click the key to see all blocks with the property, grouped by value; the view has "Include blocks below them" (the `inherit:` clause, PR-5).

- **Typing:** on a line after the first, type `::` (Logseq's flow: Shift+Enter, `::`): the menu lists the known keys, most used first; keep typing to filter. A key nobody uses yet gets "Create property: name" (also after typing `name::`). Enter writes `key:: ` and opens the value menu: values already used for the key, then pages (written as `[[Page]]`). `/property` does the same from anywhere in the block: a new `::` line goes under the block's other properties.
- **Mouse:** hover a row of the card for ✎ (edit the value in place, Enter saves, Escape cancels), × (remove the line) and, on the last row, + (add a property: name with suggestions, value with suggestions). Only that one line of the file changes. A block with no property yet gets its first one by typing (`::` or `/property`).
- **Inheritance:** properties belong to their block. Queries can apply them to the subtree: `inherit: client` (see file-format §Queries).

## Backup

Settings → Backup (this device). A snapshot is a zip of the whole graph folder (without temp files) and of this device's configuration (the device settings with the list of graphs, and the graph's window state), kept in a folder outside the graph, so it is never synced.

The page has three parts, chosen in the row under its title: **Snapshots** (*Back up now*, the list with *Restore…*, and *How to restore*, folded away until opened), **Where & when** (automatic backups, the backup folder, the second folder, compression) and **Encryption** (set up, unlock and lock, change password, new recovery key, *Forgot the password…*, *Encrypt N existing backups*). A message about the second folder or a failed backup is shown on *Snapshots*, where a backup is made.

- **Automatic backups** (on by default): one shortly after the app opens a graph and then once a day while it is open, only if something changed. The newest snapshot of each of the last 7 days, 4 weeks and 6 months is kept.
- **Backup folder:** an absolute path, empty for the default (`~/.local/share/notatnik/backups`). A folder inside the graph is refused. Put it on another disk, or in a folder another tool copies elsewhere; a snapshot on the same disk does not protect against losing the disk. Press Enter or leave the field to apply; **Use default** resets it.
- **Compression:** *Compressed* (default, about a third of the size) or *Not compressed* (files only stored in the zip: bigger, useful when the target compresses or deduplicates by itself). Both are ordinary zip files; the choice applies to new snapshots.
- **Encryption** (EN-2, EN-5, EN-6): *Set up…* asks for a password (8 to 200 characters, twice) and shows a **recovery key** once. Write it down or keep it in a password manager, away from the backups: with it, a forgotten password does not lose the backups; without both, nobody can read them. *Turn on encryption* is possible only after you tick "I saved the recovery key". From then on every new snapshot (the daily one, the one at quit, the restore points, the copy in the second folder) is sealed: the files are named `….zip.enc`, hold nothing readable, and are written **without asking for the password**. Reading one needs the password or the recovery key: *Unlock…* here, or the field that appears in a snapshot's *Restore…*. The keys stay in memory only and lock again after 15 minutes without use (*Lock* does it at once). The switch *Encrypt new backups* turns sealing off and on; while it is on a backup never falls back to plain files: if the vault file (`~/.config/notatnik/backup-vault.json`) is missing, the backup fails with a message. *Change password…* and *New recovery key…* need the current password; backups made earlier keep the password and recovery key they were made with. *Encrypt N existing backups* seals the plain snapshots in the backup folder and the second folder and removes the plain files. Your notes in the graph folder stay plain Markdown. *Forgot the password…* below the buttons resets the backup password the same way (EN-22): sealed snapshots made before stay unreadable. Without the app: `NOTATNIK_PASSWORD=… notatnik decrypt file.zip.enc` writes the zip (the password or the recovery key; without the variable it asks). To check that the server copy of a synced graph decrypts to your local files: `notatnik verify <graph folder>` (EN-20; `NOTATNIK_WEBDAV_PASSWORD` and `NOTATNIK_PASSWORD`, or it asks; exit code 2 when something differs).
- **Second folder:** optional; type a path or press *Choose a folder…* (desktop app). Every snapshot is copied there too, for example to a USB disk or a network share. The folder has to exist: while it is missing the page says so and the copies wait; they catch up at the next hourly check, at *Back up now* and when the app closes. Each snapshot shows *copied* or *not copied*. The second folder keeps the same days, weeks and months; other files in it are left alone.
- **When the app closes** a snapshot is taken if something changed and the newest one is at least an hour old (the app waits at most 20 seconds for it and for the copy).
- **Back up now** always takes a snapshot. A failed automatic backup shows its message here.
- **Snapshots:** time and size, newest first; 🔒 marks a sealed one. **Restore…** opens *Changes since this snapshot* first: every file that differs between the snapshot and now (changed, deleted since, new since, with `+n −m` line counts; + = in the snapshot, a restore would bring it back, − = now). Click a file for its lines with context, or *Show all changes*; *Hide all* closes them. Nothing is changed by looking. Below are three forms. *Device settings*: **Compare…** lists the lines that differ between the settings in the snapshot and now (+ in the snapshot, − now) and **Restore settings** puts them back; the file they replace is kept next to it as `settings.toml.before-restore-<time>`. The other two: *One page* (type the page name; the page file is replaced by its version in the snapshot, after a new snapshot of the current state was taken, so the replaced version is in the list too) and *Everything*, which unpacks into a new or empty folder you name; open that folder as a graph (Settings → Graph) to look around or copy files out. Restoring never overwrites the open graph as a whole.
- **Before a change:** a rename that touches several files and a delete of several pages first save a snapshot marked *before a change* (when automatic backups are on and files changed since the newest snapshot), so there is a restore point if the result is not what you wanted. Restoring a page does the same. The newest ten are kept.
- The settings page explains the ways to restore in *How to restore*.
- Not in a snapshot: the search index (rebuilt from the files), the trash, history copies and passwords of calendar accounts.

## Placeholders

`{{title}}`, `{{namespace}}`, `{{date}}` and `{{cursor}}` in templates, and `{{page}}`, `{{title}}`, `{{namespace}}` and `{{date}}` in queries, are replaced when a template is applied or a query runs. The full list and rules are in `placeholders.md`. In the app: **Settings → Templates → Placeholders** has the table, and typing `{{` in a query block offers the query placeholders.

## Templates for page types

A page type has a template: the file `templates/<type>.md` in the graph folder (`templates/person.md` for `type: person`). Its properties are the fields a page of that type should have (`partner:` with no value shows an empty field), its blocks are the starting content, and `{{title}}`, `{{namespace}}`, `{{date}}` are filled in.

Setting `type` on a page adds the template's properties that the page lacks, and, when the page has no blocks yet, the blocks. A value you already set is never changed. It is one edit, so undo takes it back in one step, and a message names what was added. When a template later gets a new field, a page of that type shows **Add missing from template “person” (email)** under its properties. *Settings → Templates* turns the automatic step off (the button stays).

**Making a template:** set up one page the way you want (`type`, properties, blocks), open its properties and click **Save as template…**. Choose *Fields empty* or *Keep these values as defaults* and **Save**; if a template of that type exists you are asked to **Replace it** (the old text stays in the history of the file). The properties `title`, `aliases`, `archived` and `template`, and multi-line values, are left out; block IDs are not copied. *Settings → Templates* lists the templates with their fields and **Delete…** (asks again).

**New template:** *Settings → Templates → Your templates → New template…* asks for the page type (`person`) and makes `templates/person.md` with the `type` line only, then opens it for editing. A name that exists already (in any case) is refused; a name with characters that mean something in YAML (`:`, `#`, …) is written quoted. Pages with that `type` start from it.

**Seeing and editing a template:** *Settings → Templates → Your templates* lists each template with its fields and block count. *Edit* opens the file's text as it is (the properties between the `---` lines, then the blocks) in a text box; *Save* (or Ctrl+Enter) writes it, *Close* or Escape leaves, and *Discard* appears while there are unsaved changes. The text that a save replaces stays in the file's history. Pages made afterwards start from the new text; pages that exist are not changed.

**New pages and days:** in *Settings → Templates* pick a **Journal template** (every new journal day starts from it, `{{date}}` is that day) and add **namespace templates** (`people` → `person`: every new page below `people`, not `people` itself, starts from `person`: its `type`, properties and blocks). The longest matching namespace wins. What you type first comes after the template's blocks. The switch *Apply templates automatically* covers these too.

**/template:** type `/template` in a block and pick **Template: person**. In an empty block the template's first block takes its place and the rest follows; in a block with text the template's blocks are inserted after it. The page also gets the template's `type` and properties that it lacks (an empty field in the template stays an empty field; values you already have are kept). `{{title}}`, `{{namespace}}` and `{{date}}` are filled in for the current page.

**Saving blocks as a template (snippets):** select blocks (Esc in a block selects it; Shift+↑/↓ or Ctrl+click for more), then right-click and choose **Save as template…**, or press Ctrl+K and choose it there (with no selection it takes the block being edited). A selected block takes its children along. The dialog shows the blocks and asks for a name; **Save** writes `templates/snippets/<name>.md`, only the blocks, no page type. A name that exists asks **Replace it** (the old text stays in the file's history). `/template` then lists it as **Snippet: name** next to the page types; it inserts the blocks after the block (in an empty block the first one takes its place), with `{{title}}`, `{{namespace}}` and `{{date}}` filled in, and adds no page properties. *Settings → Templates → Saved blocks* lists them with *Edit* (the text as it is), *Delete* (30 days under *Deleted templates*, with *Restore*). To leave out the first block, select only its children.

**Deleting and restoring:** *Settings → Templates → Delete* removes the file; it is kept for 30 days on this device under *Deleted templates* with **Restore**. Restoring is refused while a template of that name exists.

## Pages as property values

A property can hold a page. In a page's *properties* (the `▸ properties` section under the title) and in the card under a block, the value field suggests the values already used for that property and, below them, pages that match what you type (marked *page*). Arrow keys and Enter pick, a click picks, Esc closes the list. A picked page is written as a link: `partner: "[[part. Capgemini]]"` in the page's frontmatter (quoted, because YAML would read bare `[[x]]` as a list), `partner:: [[part. Capgemini]]` in a block. The value then shows an `→ part. Capgemini` link that opens the page; type `[[A]], [[B]]` for several.

The **Namespace** and **Tag** fields of the Agenda suggest pages in the same way; a pick is written as the plain name.

In a query block, `prop.partner: ` offers the same list: values in use and pages, a page is inserted as `[[part. Capgemini]]`. `prop.partner: [[part. Capgemini]]` finds the pages and blocks whose `partner` is that page; `-prop.partner: ...` excludes them, `prop.partner: any` / `none` test for presence.

## Page history

*History* next to *Archive* under a page's title (journal days have it too). It lists the versions of the page's file recorded on this device, newest first. Click one to see what restoring it would change (red: the page now, green: that version) and press *Restore this version*; the text it replaced becomes a version, so a restore can be undone from the same list. Versions are recorded whenever the page is saved or changed from outside (sync, another editor); the text before the first change is kept. Typing is one version per minute; older versions thin out to one per 10 minutes for 3 hours, per hour for 2 days and per day for 30 days. The history is on this device only and follows a renamed or moved page.

## If the app stops in the middle of a rename or delete

A rename (with the links it rewrites), a namespace move and a delete of several pages are written to a journal before they start. If the app is killed or the computer loses power half way, the next time the graph opens the unfinished operation is undone and a warning says what was put back. A single page save needs none of this: it is replaced atomically.

## Grouping pages by a property

The **Pages** part of the left panel has a picker: *by namespace* (the tree) or *by `<property>`* for every page property in use. With
`type: person` / `type: client` in the pages' frontmatter you get a **person** and a **client** group with their pages. A page with a list
(`teams: [red, blue]`) is in each of its groups; values that differ only in case are one group; pages without the property are in a group
at the end. The tree and the groups **start collapsed**; the **⊟ Collapse all** and **⊞ Expand all** buttons next to the *Pages* title fold or open every branch or group (what you opened by hand afterwards is remembered, per device). Open a page's properties section (the `▸ properties` line under the page title; it is available on journal pages too, but
not on the journals landing page, neither in today's entry nor in the compact day entries: click a day's title to open it as a page first) to add, change or remove properties; a value that YAML could
misread (`a: b`) is quoted automatically.

**Click a property** to see the pages that have it: the property name opens `#/property/<key>` (all pages, grouped by value), and the `↗ value` link after each value opens `#/property/<key>?value=<value>` (only those pages, with the other values as links; matching ignores case). Lists such as `[red, blue]` get one link per item.

## Moving blocks with the mouse

Drag a block's **bullet**. The block moves together with all its sub-bullets. The drop line shows where it lands: upper half of a block = before it; lower half = after it, or as its first child when that block shows children, or as a child when you drop clearly to the right of its bullet. A block can't be dropped into itself. Undo (Ctrl+Z) brings it back. Dragging works within a page (moving between pages is not built yet).

## Linking files

- `/file` ("Link a file or folder…") opens a picker under the cursor and inserts a Markdown link `[name](address)` where the command was typed. Esc steps back one level; Esc on the first level closes it.
- **A server:** each server in the list opens as a file browser: breadcrumbs, folders, files with their size, a filter box (Enter takes the only match). Click a file to link it. A folder row has a *Link* button that links that folder, and *Link this folder* above the filter links the folder you are in (at the top: *All files*, the Files app). A folder is linked, never copied. *Add a server…* asks for a name, the server, user name and password (on Nextcloud an app password); it logs in once, so a wrong password is told at once. The server the open graph is synced with is always in the list.
- A link to a Nextcloud file is its `…/f/<number>` address: it opens the file in Nextcloud's web interface and keeps working when the file is renamed or moved. A server that gives no file numbers gets the file's own address, which breaks when the file is renamed or moved (the picker says so).
- **This computer:** in the desktop app a file chooser opens, then *Link to the file where it is* writes `[name](file:///home/me/…)` (works only on this computer) or *Copy it into the graph* stores the file as `assets/YYYY/MM/<name>-<hash8>.<ext>` (synced, up to 25 MB, programs and scripts are refused) and links it by a relative path. *Link a folder instead…* opens the system's folder chooser and writes `[name](file:///home/me/folder)`; clicking it opens the folder in the file manager. In a browser only the copy of a file exists.
- **Type an address:** any `https://`, `http://`, `mailto:` or `file:///` address, with optional link text.
- Clicking such a link opens it in the system browser (a web address) or with the system's program for the file (`file:///…` and copied files). Programs and scripts are never opened from a note.
- Settings → **Linked files** lists the servers (this device only; the password is in the system keyring) with *Edit*, *Remove…* and *Add a server…*.

## Calendar (Settings → Calendar)

- **Login:** a graph that is synced with a Nextcloud uses the same login for its calendars (*Connect and list calendars*). *Use another login…* (or a graph with no sync) asks for the server, user name and an app password; it is checked before it is kept, and the password is stored in the system keyring, never in a file.
- **Calendars:** after connecting, each calendar of the account is listed. Choose the one that *receives tasks* (only calendars that take tasks can be chosen) and the calendars whose events are shown. *Calendar sync is on* and *Save* keep the choice (this device). Whether tasks are tasks or events and what happens to a deleted task is the graph's `[calendar]` section.
- **Sync tasks with the calendar now** (also *Ctrl+K → Sync tasks with the calendar*): first reads what changed in the calendar, then sends what changed in your notes. Every task with a `scheduled::` or `deadline::` date is sent as a calendar task (VTODO): title without marker, links and `^id`; state, dates (with a time of day as floating local time), priority, `repeat::` as a repeat rule, tags as categories, plain child lines as the description, and a `notatnik://page/…` link back. The first time, a dated task gets a block ID (` ^abc123`) at the end of its line; nothing else in your files changes. `cal:: no` on a task keeps it out. A second run with nothing changed sends nothing.
- **Which calendar a task goes to**: the default one (*Receives tasks* in Settings → Calendar), unless the task has `cal:: <name>` with the name of another calendar you ticked under *Also takes tasks* (case does not matter; an unknown name means the default; `cal:: no` keeps it out). Every task the sync would send shows a **Calendar** chip with the name (*not synced* for `cal:: no`) on pages, in the agenda and in query results; click it and choose another calendar, the default, or *not synced*, which writes or removes the `cal::` property. Changing it moves the task: it is deleted from the old calendar (not completed) and sent to the new one. A task made in another of your task calendars is copied into the notes with `cal:: <that calendar>` and stays in that calendar. Tasks shown in a compact row have the chip in the details popover.
- **Tasks without a date** are not sent unless you turn on Settings → Calendar → *Also send open tasks without a date* (`[calendar] undated = "open"`). Then every open task (`TODO`, `DOING`, `NOW`, `LATER`, `WAITING`) is in the calendar, as an item with no date, and gets a block ID on its line; finished and canceled tasks, and tasks with `cal:: no`, stay out. A task finished in the notes or in the calendar is completed on both sides. Turning the setting off completes the undated items in the calendar (after two syncs, like any task that left the notes).
- **Changes made in the calendar** (phone, Nextcloud Tasks, Thunderbird) are written into the block: completed or canceled (`DONE`, `done::`; a repeating task moves on as when you complete it), started, `scheduled::`, `deadline::`, `priority::`, `repeat::`, and the title when the first line is plain text. A title with a link, tag or formatting is never rewritten: the sync log says the calendar's title differs. When you and the calendar changed the *same* field, the later change wins (the page file's time against the item's last-modified time; a tie goes to the notes) and the sync log shows both values. Fields changed on only one side, and different fields on both, are all kept.
- **A task deleted in the calendar** stays in your notes with its dates; the sync log says so, and it is not sent again. Add `cal:: no` to stop it for good.
- **Copy tasks made in other apps into my notes** (Settings → Calendar, off by default). Open tasks that another app created in a task calendar become `TODO` blocks with their dates, priority, repeat and notes, either **in today's journal under the heading `### Synced tasks`** (the heading is made when it is missing and reused afterwards; its name is `[calendar] import_section`) or on the page `inbox` (`[calendar] import = "journal"` or `"inbox"`, chosen with the *Copy them to* list). A copy with a date is sent back under its own item and the original is removed; one without a date leaves the original in place.
- **A task is copied once, whichever device syncs.** Each copy has a `cal-source:: <the other app's task ID>` property and a block ID made from it. Before copying, a device looks for that property anywhere in the notes, and it marks the original in the calendar as taken (`X-NOTATNIK-IMPORT`, written with the item's ETag, so only one device succeeds). Another device leaves a task alone that was taken within the last day; once the copy is made, an original that stays carries a final mark, so no device copies it again, even when you delete or move the copy. Some apps drop the mark; then the property in the notes keeps devices apart once a device has the other's notes.
- **Moving earlier copies from the inbox into the journal**: when you switch the copy target to the journal and the page `inbox` still holds copies the sync made, Settings → Calendar offers *Move them into today's journal*. Only blocks the sync made move (found from the sync's records and log), with their IDs, dates, notes and children; anything else on the page stays. Copies that are somewhere else only get their `cal-source::` mark. If the move cannot finish (a page changed meanwhile) nothing is left half done. The page history brings an old version back.
- **Automatic sync**: with calendar sync on, the app syncs by itself when a graph opens, every 5 minutes, and 10 seconds after the last change to your notes, while it is open. After a failure it waits 5, 10, 20, then 30 minutes; Settings → Calendar shows the last result. A quiet sync is one request and writes nothing.
- **Events**: the calendars ticked under *Show events of* are read (read only) and shown in a *Calendar* section under each journal day, and beside the tasks in the timeline view of a query. Repeating events, a deleted or moved occurrence, all-day and multi-day events and other time zones are handled; times are shown in your local time. Events are never written into your files.
- **Pause**: Settings → Calendar → *Pause calendar sync* (a switch, saved at once, no reconnecting needed), or Ctrl+K → *Pause or resume calendar sync*. While paused nothing is sent to or read from any calendar, the automatic sync does not run and *Sync now* is refused; your calendars and choices are kept, and the first sync after you resume brings everything in step (a task you deleted meanwhile leaves the calendar only after two syncs, as always). The setting is per device and graph and is not written into the graph.
- **Sync log** (Settings → Calendar, under the sync button): the last 50 entries, newest first. A conflict entry has a link, "Use the notes value instead" or "Use the calendar value instead": it writes the value that lost into the block and syncs, so the calendar gets it too. It is offered only while that entry is the latest for the task and field.
- A calendar item that was changed elsewhere (phone, Nextcloud Tasks) since it was sent is not overwritten; the run tells how many (reading such changes back into the notes comes next). A task that is deleted, loses its dates or gets `cal:: no` is marked done in the calendar (or removed: the graph's `[calendar] on_delete = "delete"`), but only after it has been missing in two runs.
- *Remove…* forgets a login of its own and turns the sync off. Reading changes back into the notes and events in the journal are the next steps of phase 4; there is no automatic or background sync yet.

## Emojis

- Type `:` and a word (`:rocket`, `:check`, `:fire`) for suggestions; Enter inserts the emoji itself. Times like `12:30` and `key::` properties are not affected.
- `/emoji` opens a picker under the cursor: type to search, Enter takes the first match, Esc closes it.
- **Page icons:** Settings → Page icons has rules such as `type = person → 👤` (synced with the graph). A page can set its own icon with `icon: 🏢` in its properties. Icons show next to the page in titles, lists, the sidebar, the hierarchy and links.

## Recent pages

The sidebar's **Recent** list shows the last pages you opened (newest first, up to 10; remembered on this device, separately for each graph). *clear* empties it; renaming a page keeps its place.

## Pages

`All pages` (sidebar, or the palette entry "Go to all pages") lists every page: **List** (filter, sort by name, blocks, linked-from count or
modification time; journals and pages without a file can be hidden) or **Namespaces** (the tree).

## Query blocks

A ```` ```query ```` block shows its results live. To change the query, click anywhere on the block except the results: the header (it says "✎ click here to edit the query"), the frame around it or the text shown by *show query* opens the block for editing. A click on a result keeps its own meaning (editing that block, opening a card, ...).

**This page:** `{{page}}` in a query stands for the page the query block is on. On the page `part. Capgemini`, `type: page` + `prop.partner: {{page}}` lists every page whose `partner` is that page; the same block pasted into another partner page lists that partner's pages. It works wherever a page is allowed (`page:`, `links:`, `prop.x:`; `[[{{page}}]]` is the same). `{{title}}` is the last part of the name, `{{namespace}}` the part before it, `{{date}}` today. In a saved query (sidebar) and in the agenda there is no page, so `{{page}}`, `{{title}}` and `{{namespace}}` are reported as an error. The older `[[]]` still works.

## Tasks of a page

In *In subpages* and *Mentioning this page* every task shows its source as a chip at the right end of its first row (the same as in the agenda): the name below this page in subpages (`Example` for `Conference/Example`), the date in your date format for a journal day, the full name otherwise; the tooltip has the full path of parent blocks and a click opens the note with the task flashed. In *On this page* the chip is the parent blocks (when there are any) and shows the task in the page.

A page that has tasks shows a **Tasks** section between its content and its references: *On this page* (its open tasks in one list, also those under collapsed blocks), *In subpages* (for a namespace page) and *Mentioning this page* (tasks elsewhere that link to it, e.g. everything open about a client). Tick **Show done** to include finished ones. A task can be changed right there; `↗` opens the note and flashes the task. The section is hidden when there is nothing to show.

## Hierarchy

A page with subpages ends with a **Hierarchy** list: every page below it at any depth as its path (`Z / Partner Beta / Renewal`), every segment a link to that page.

Every page has the **Hierarchy** section, also without subpages. **+ Add subpage** at its end (or the palette command **New subpage of this page**) opens a name field prefilled with `Page/`; type the rest and press Enter to open the new page ready for typing (the file is written when you type the first block). Escape cancels. A subpage of an encrypted folder shows a 🔒 next to the field.

## Topics: a name that occurs in several paths

Every segment of a page name is an automatic, indirect tag. With `P/Capgemini/Renewal/Siemens Energy`, `P/EY/Renewal/APotheke` and `X/Renewal/Snowflake`, the Hierarchy of `P` shows **Renewal³**: the count means it occurs in three places. Click it for the **Renewal** view:

- the places, grouped (`P/Capgemini/Renewal`, `P/EY/Renewal`, `X/Renewal`), each with the pages below it; the group headers link to the pages themselves;
- the **open tasks** of all those pages (tick *Show done* for the rest);
- the **linked references** to a tag or page of the same name (`#renewal`, `[[Renewal]]`), so explicit tags and path segments end up in one place.

The same count and link appear everywhere a name of a path is shown, also for the page you are on: in its **Hierarchy**, in the path above its title (`P / Capgemini /`) and as a small **in 2 places** link next to the title of `P/Capgemini/Renewal`. So from `P/Capgemini/Renewal` one click on `Renewal` lists all renewals. Unique segments (no number) open their page; the group headers in the Renewal view link to each specific page. In queries use `path: Renewal` (also `-path:`, `path: a, b`, `path: ey/renewal` for two segments in a row), e.g. `task: todo` + `path: Renewal`.

## Archiving and deleting pages

Under a page's title: **Archive** and **Delete…** (not on journal days: those are deleted by their file).

- **Encrypted pages in All pages (List):** a 🔒 after the name (also in the **Namespaces** view, in the sidebar's page tree and in **Recent**) marks a page that is stored encrypted on the server (its file is covered by a marked path). The checkbox **Only encrypted** (shown once something is marked) lists just those, in both views; in the **Namespaces** view the folders above an encrypted page stay and are opened.
- **From All pages (List):** each row ends with **Archive** (or **Unarchive**, for a page that carries the property itself) and **Delete…**. They do what the page's own buttons do, and you stay on the list. Archiving and unarchiving show a toast with **Undo**; **Delete…** opens the same confirmation as on the page (links, pages below), and its Undo restores the files. The **Namespaces** view of All pages has the same buttons on each page (archived pages are not in that tree, so it has no Unarchive). Rows of journal days and of pages without a file have no buttons.
- **Archive** sets `archived: true` in the page's frontmatter. The page and every page below it leave the sidebar tree, the property groups and Recent, and their tasks leave the agenda. The page stays reachable by link, search and the **Archived** checkbox of All pages, and shows an "Archived" banner with **Unarchive**. Nothing is moved, so links keep working and the file syncs like any other. Queries are not filtered; add `-prop.archived: true` to leave archived pages out of one.
- **Delete…** moves the page file to the trash and offers **Undo** for a while (the trash is in the device data folder, not in the graph, and is emptied after 30 days). Links to the page stay and then point to an empty page; a dialog says how many pages link to it. A namespace page can take its subpages with it (tick the box); one **Undo** restores them all. The page's undo history is dropped.

## Renaming and moving pages

Change one segment of a page's path (`P/CapGemini/sth` → `P/Capgemini12/sth`; the last segment unchanged) and the dialog offers **Rename `P/CapGemini` to `P/Capgemini12` for all pages below it**, ticked by default: the namespace is renamed, so the pages beside it, their subpages and every link and tag follow, and you stay on the page you opened. Untick it to move only that page. If a page of the new name already exists the rename is refused. If you also change the last segment, or several segments, it is a plain move of one page. Config text is not rewritten (a `path: CapGemini` clause in a saved query, a page-icon rule).

Rename (title → Rename) moves a page; if it has subpages they move with it and all links and tags follow (preview first). Untick **Move the subpages too** to rename only this page; that also lets you give it a name below its own (`a` → `a/b/c`). If a file changes while the move runs, **Revert** restores what was already rewritten.

## References

A reference shows the linking block **with all its sub-bullets**, editable in place, and the path of its ancestors above it (each a link that opens that page at the block). If a bullet and one below it both link the page, only the outer one is listed. Because a link applies to everything below it, a `TODO` under a bullet `[[Client One]]:` also shows in Client One's **Tasks** section ("Mentioning this page"), in `links: Client One` queries and in `tag:` filters of the agenda; add `direct: yes` to a query to match only bullets that hold the link themselves. Dates and other properties are not inherited.

A page ends with **Linked references** (blocks and pages that link to it) and **Unlinked references** (plain-text mentions of its name or
aliases outside links, code and tags). Open the unlinked panel to search; **Link** turns the first mention in that block into `[[Page]]` (or
`[[Page|mention]]` when written differently) and can be undone.
