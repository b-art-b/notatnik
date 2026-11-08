# Android widgets: quick capture and today's tasks (AW1 to AW6)

**Goal:** on the phone's home screen, a **capture bar** that adds a line to today's journal in two taps (without waiting for the
app to load), and a **list of today's tasks**. Both are native Android widgets: a widget cannot hold a web view.

- **Capture bar** (4×1): shows _Add to journal…_. A tap opens a small capture dialog over the home screen; _Add_ puts the text in a
  queue and closes it. The app appends the queued lines to the graph the next time it runs (starts or comes to the
  foreground), and its normal sync sends them on.
- **Today's tasks** (4×2, resizable, scrolls): the tasks due today and the overdue ones, most urgent first; a tap on a row opens
  its page in the app. Read-only: no checkbox (a later step).
- **Capture screen in the app** (the part of P8.12 the widget needs): the route `#/capture` opens straight to an input and
  appends to today's journal; the bar's overflow action _Open in app_ and the deep link `notatnik://capture` lead there.

**Requirements covered:** AN-2 (quick capture), new AN-5 to AN-7 (see `requirements.md`); P8.12 of `phase-8.md`.
**Depends on:** the Android shell and sync (`android-sync.md`, AS4: sync on resume), the device folders of the app
(`device_dirs`, below the app's data folder on Android), the agenda/task queries, encryption (EN-\*: encrypted pages).
**Status:** AW1 to AW5 built (Rust and browser tests pass; Kotlin builds, see each Verify); the checks on a device (AW6) are open.

## Decisions

- **Native Kotlin, no web view.** An `AppWidgetProvider` per widget, XML layouts, `RemoteViews` (a `RemoteViewsService` for the
  scrolling list). Files go into the tracked `src-tauri/gen/android` next to `SecretsPlugin.kt`; the manifest gets the
  `<receiver>` entries and an exported-false capture activity.
- **The widget never touches the graph's Markdown.** One writer only: the app, through its normal edit path (atomic writes,
  ST-3, conflicts, undo, the index). The widget only reads a snapshot and appends to a queue.
- **Both files are device data, not graph data (ST-2):** `<app data dir>/widgets/capture-queue.jsonl` and
  `<app data dir>/widgets/today.json`. They are in app-private storage, readable by the widget because it runs with the app's
  user id. They are never synced and never in a backup of the graph.
- **The queue.** One JSON line per capture: `{id, graph, text, at}` (`id` random, `graph` the graph's slot name, `at` the
  moment of capture in local time). The dialog appends with a single `O_APPEND` write of one whole line, so a half-written
  line is not possible and a crash loses at most the line being typed.
- **Draining.** The UI calls `widgets_sync` when a graph is open, on every return to the foreground, when the app goes to the
  background and three seconds after a page was saved; when it added lines it starts a sync right after (the sync that
  `SyncStore.resume` starts may run first; the lines then go with the next one, seconds later). `widgets_sync` renames the queue to `capture-queue.draining`, appends each line as a **top-level block at the end of that day's
  journal** (the day of `at`, so a note typed at 23:58 and drained at 00:10 lands on the right day), and deletes the file
  after the last one is written. Ids already applied are remembered (the last 200, in device state), so a crash between
  writing and deleting never adds a line twice. A line for a graph that is not open is kept until it is; one that fails to
  apply is kept in `capture-queue.failed` and shown once, never dropped silently.
- **Which graph.** The one that was active when the widget was placed or last updated (the snapshot names it); with two
  graphs open the widget follows the **first** graph unless a later task adds a choice. The capture target
  is always **today's journal** in this phase; the `capture` command takes a page, so an inbox page setting is a small step later.
- **The snapshot** `today.json`: `{v:1, graph, date, written_at, tasks:[{page, text, due, overdue, marker}], more}`, at most
  20 tasks, today's first, overdue after, then by priority and page. Written by the Rust side after the index changes
  (debounced to at most one write per 2 s, atomically through a temp file and rename) and when a sync brought changes.
  **Tasks of encrypted pages are left out** (`encrypted` pages are never copied to a place outside the graph's own
  protection), and so are tasks of archived pages. The snapshot holds only task text and the page name.
- **Staleness.** The widget shows the snapshot's `date`; when it is not today the list says _Open the app to refresh_ instead of
  showing yesterday's tasks as today's. A missing or unreadable file shows _Open Notatnik to set up_.
- **Refreshing.** The Rust side asks the Kotlin plugin to refresh the widgets (`AppWidgetManager.notifyAppWidgetViewDataChanged`)
  after writing the snapshot; `updatePeriodMillis` (30 min) and the date change broadcast are the fallback.
- **Opening the app.** Taps use intents that carry a `notatnik://page/<name>`, `notatnik://capture` or `notatnik://agenda` link
  (the manifest also lets `adb shell am start -d notatnik://…` use them);
  `MainActivity` (launch mode `singleTask`) turns it into the hash route in `onCreate` and `onNewIntent`. A link is never
  trusted: a page name is looked up like a typed one and an unknown name shows the usual "no such page".
- **Look.** Follows the system light/dark theme and the font scale; rows are at least 48 dp for the thumb; text is
  ellipsized, not clipped mid-letter; the widgets declare a description for the picker and content descriptions for the
  rows (TalkBack).
- **No new Android permissions.**

## Tasks

### AW1 The capture screen and the deep links in the app

- Route `#/capture` (and the command _Capture_): the page is one focused input, _Add_ and _Cancel_; _Add_ appends a top-level
  block to today's journal (or the target setting) and goes back to where you were. `MainActivity` maps the intents
  `notatnik://capture` and `notatnik://page/<name>` to routes (also when the app is already running).
- **Verify:**
  - [ ] `#/capture` opens with the input focused; typing and _Add_ puts one block at the end of today's journal and the
        journal shows it; _Cancel_ writes nothing
  - [ ] an empty or whitespace-only line adds nothing and says so
  - [ ] the `capture` command with a page name puts the block there, and the page is created when missing (Rust test)
  - [ ] a browser test (Chromium, a phone-sized viewport) covers the three lines above; the same in WebKit
  - [ ] emulator: `adb shell am start -a android.intent.action.VIEW -d notatnik://capture eu.cdfg.notatnik` opens the screen,
        also when the app is already open on another page; `notatnik://page/Home` opens that page
  - [ ] a link with a name that does not exist, or with `..` or a very long name, does nothing harmful

### AW2 The capture bar widget (4×1) that opens the capture screen

- `CaptureWidgetProvider`, layout, `appwidget-provider` XML (4×1, resizable horizontally, not scrolling), manifest receiver.
  A tap starts `MainActivity` with `notatnik://capture`. This is the first usable widget and needs no queue.
- **Verify:**
  - [ ] the widget is in the picker with its description and a preview; it can be placed and resized on the emulator and on
        the phone
  - [ ] a tap opens the capture screen of AW1 in the app, from the home screen and with the app already running
  - [ ] light and dark theme, 130% font scale: nothing is cut off
  - [ ] removing the widget leaves no error in `adb logcat`

### AW3 Capture without opening the app: the dialog and the queue

- A translucent `CaptureActivity` (own task, no app launch, keyboard up at once) writes to `capture-queue.jsonl`; the Rust
  side drains it (the rules under _Decisions_) at start and on resume, before the sync; the bar's tap now opens this dialog,
  _Open in app_ in the dialog goes to AW1's screen.
- **Verify:**
  - [ ] Rust tests: a queue of three lines becomes three blocks in the right order at the end of the right day; a line stamped
        before midnight and drained after goes to the earlier day; a repeated drain after a crash (file kept, ids known) adds
        nothing twice; a line for another graph stays; a broken line goes to `.failed` and the rest are applied
  - [ ] Rust test: the drain goes through the normal edit path (the file is changed by a minimal text replacement, undo and
        the index see it) and creates the journal file when the day has none
  - [ ] Kotlin unit test (JVM): a line is one JSON object ending in `\n`; text with quotes, newlines (sent as a space or
        kept as a soft line break, decided in the test), emoji and 5000 characters survives the round trip
  - [ ] emulator: type in the dialog with the app **closed**; the dialog closes in under a second; opening the app shows the
        line in today's journal and a sync sends it (the file is on the server)
  - [ ] emulator: five captures in a row without opening the app; all five arrive in order
  - [ ] airplane mode: the same, and the sync catches up when the network is back
  - [ ] two graphs: the line lands in the graph named in the queue entry

### AW4 The snapshot of today's tasks

- Rust: `widgets::write_snapshot` called after index changes (debounced) and after a sync; a Tauri command and a Kotlin plugin
  method `refreshWidgets`. The query is the agenda's "today and overdue" with the same ordering.
- **Verify:**
  - [ ] Rust tests: a graph with tasks due today, overdue, undated, done and in an archived page gives exactly today's and the
        overdue ones, in order, capped at 20 with `more` set; a task on an encrypted page is not in the file
  - [ ] the file is replaced atomically (a reader never sees half a file); a burst of ten edits writes it once or twice
  - [ ] with no graph or an empty one the file says so (no crash, no stale tasks)
  - [ ] `docs/file-format.md` or `architecture.md` names the two widget files as device data (ST-2), not part of the graph

### AW5 The tasks widget (4×2)

- `TasksWidgetProvider` with a `RemoteViewsService` list (rows: marker, text, page name small); a row's tap opens the page via
  `notatnik://page/<name>`; the header's tap opens the agenda; the states _Nothing due today_, _Open the app to refresh_ and
  _Open Notatnik to set up_.
- **Verify:**
  - [ ] on the emulator with a 20-task graph: today's tasks are listed with the overdue ones marked, the list scrolls, a row
        opens its page
  - [ ] the widget changes without opening the app: after a task is edited on the desktop and synced, opening the app once
        updates the widget within a few seconds; after the date changes the list says _Open the app to refresh_
  - [ ] light/dark, 130% font scale, a 4×2 and a 4×4 size; TalkBack reads each row as "task, page"
  - [ ] a task on an encrypted page never appears (checked on the device by reading `today.json`)
  - [ ] battery: no wake-ups beyond `updatePeriodMillis` and the date change (`adb shell dumpsys batterystats` or the
        alarm list shows nothing else)

### AW6 Real phone, docs

- The arm64 release APK on the phone: place both widgets, capture for a day, restart the phone, update the app over the
  old one (the widgets survive).
- Docs: `keymap.md` (the widgets, the capture screen, the setting), `install.md` (Android is built; the stale "not available"
  text), `architecture.md` (the two device files), `phase-8.md` (P8.12 ticked as far as built), `testing.md`, `TODO.md`.
- **Verify:**
  - [ ] the whole of AW2 to AW5 on the phone, not only the emulator
  - [ ] after a phone restart and after an app update the widgets still work and the queue is intact
  - [ ] the docs above are updated; `scripts/check.sh` passes (the Rust and browser parts) and the Kotlin unit tests run from
        `scripts/android.sh`

## Not in this phase

- A checkbox in the tasks widget (marking DONE through the same queue, applied by the app): a later step.
- A journal or pinned-page widget, a search widget, a lock-screen or Quick Settings tile.
- Choosing the graph for a widget with two graphs open; widgets on iPad/iOS.
