# Phase 4: Calendar integration (two-way CalDAV with Nextcloud)

**Goal:** dated tasks appear in your Nextcloud calendar and task apps, changes made there (on the
phone, in Thunderbird) flow back into the Markdown files, and calendar events show up in journals,
all without ever losing or duplicating data.

**Requirements covered:** CA-1–9, JR-5, QV-6 (events in the timeline).
**Depends on:** phase 3 (tasks, repeats); spike S3 findings.

## Exit criteria

1. Every task checked, with every **Verify** item passing; `scripts/check.sh --full` passes, including the Radicale integration suite.
2. The compatibility matrix (P4.14) is fully green, or each red cell has a documented workaround.
3. Two weeks of real use with your Nextcloud: zero duplicate calendar items, zero lost edits (check the sync log).

## New crate

`crates/caldav` (`notatnik-caldav`): CalDAV client wrapper, iCalendar mapping, sync engine. The sync
engine is pure logic over a `CalDavClient` trait, so most tests use an in-memory fake server and a
subset runs against Radicale.

## Key design decisions for this phase

| Topic | Decision |
|---|---|
| Item identity | UID = `notatnik-<graph-id>-<block-id>` (file-format.md §7). Dated tasks get a block ID automatically when sync is enabled (`EnsureBlockId`). |
| What syncs back | Status, DTSTART/DUE, PRIORITY, RRULE and COMPLETED always sync back. SUMMARY syncs back **only if the block's first line has no inline markup** (links, tags, refs, formatting); otherwise the remote change is logged as a conflict, because a plain-text summary can't be written into rich Markdown without loss. |
| Sync state | Per item, the cache stores the *last synced state* (normalised fields + ETag). A three-way comparison (base vs local vs remote) decides who changed what. If the cache is lost, the first sync compares local vs remote directly: equal fields → nothing; different fields → local wins for content, remote wins for status (configurable). |
| Loop prevention | A pull that writes into a block updates the base state at the same time, so the next push sees no difference. |
| Deletion safety | A block missing from the index does **not** immediately delete the remote item. Deletion only happens when (a) the full index is complete and settled (no reindex in progress, no conflict files for that page), and (b) the block has been missing for two consecutive sync runs. Default `on_delete = "complete"` marks the VTODO as completed instead of deleting it. |
| Externally created VTODOs | Tasks created in another app (no `notatnik-` UID prefix) are imported into an inbox page (`inbox` by default) as new blocks, which then get IDs and are re-uploaded under a notatnik UID, and the original is deleted after a successful upload. Configurable: off / import. |
| Time zones | Date-only values → `VALUE=DATE`. Datetimes → local time with `TZID` = the system time zone, with a VTIMEZONE included. Reading: convert any TZ to local time for display; keep the original TZID to avoid needless rewrites. |
| Multiple devices | Several devices may run sync for the same graph. Writes use `If-Match` ETags; a 412 response → pull, re-merge, retry. Because the UID and content are derived deterministically from the file, two devices pushing the same state is harmless. |

### Field mapping (block ↔ VTODO)

| Markdown block | VTODO | Notes |
|---|---|---|
| First line text (markers, `^id` and properties removed; links reduced to their labels) | `SUMMARY` | See the SUMMARY write-back rule |
| `TODO` / `DOING` / `WAITING` / `DONE` / `CANCELED` | `STATUS` `NEEDS-ACTION` / `IN-PROCESS` / `NEEDS-ACTION` + `X-NOTATNIK-STATE:WAITING` / `COMPLETED` / `CANCELLED` | |
| `scheduled::` | `DTSTART` | |
| `deadline::` | `DUE` | |
| `priority:: high/medium/low` | `PRIORITY` 1 / 5 / 9 | Reading: 1–4 → high, 5 → medium, 6–9 → low, 0 → none |
| `repeat::` | `RRULE` | Unsupported remote RRULEs are kept in `repeat:: RRULE:…` verbatim |
| `done::` | `COMPLETED` | Non-repeating tasks |
| last date in `completed::` | — | Repeating tasks: completing an occurrence in a client (or the app) advances DTSTART/DUE to the next occurrence; the completion date is appended to `completed::` |
| Child blocks (plain text) | `DESCRIPTION` | Push only (not synced back) |
| Page name + link | `X-NOTATNIK-PAGE`, and `URL` with an app link (`notatnik://page/<name>#<id>`) | So the phone can show where the task lives |
| — | `CATEGORIES` | Tags of the block (push only) |

When `tasks_as = "vevent"`, the same mapping applies to VEVENT (status via `X-NOTATNIK-STATE`; DTSTART = scheduled or deadline).

---

## 4A: Test infrastructure

### P4.1 Radicale for automated tests
- [x] `pipx install radicale` (lightweight CalDAV server); a test helper starts it on a random port with a temp storage folder and a test user
- [x] The integration test suite runs only with `--full` (or when `RADICALE` is available)

**Verify**
- [x] `cargo test -p notatnik-caldav --features integration` starts Radicale, creates a calendar, runs a PUT/GET, and stops Radicale (no leftover process: `pgrep radicale` is empty afterwards)

### P4.2 Nextcloud in Docker for compatibility tests
- [ ] `docker compose` file in `dev/nextcloud/` (Nextcloud + DB), a script that creates a test user, calendars and an app password
- [ ] Your real Nextcloud is used only after this passes, with a dedicated test calendar first

**Verify**
- [ ] `docker compose up` → the Nextcloud web UI is reachable at `localhost:8080`; the Calendar and Tasks apps are installed; the script prints working credentials
- [ ] The S3 spike binary works against this instance

### P4.3 Fake CalDAV server for unit tests
- [x] An in-memory implementation of the `CalDavClient` trait with ETags, sync-tokens, conditional writes (`Error::Changed`, the 412) and fault injection (offline, 500; `crates/caldav/src/fake.rs`)

**Verify**
- [ ] The fake passes the same contract tests as Radicale (one shared test suite run against both)

## 4B: Account and discovery

### P4.4 Account setup (CA-1)
- [x] Settings → Calendar: server URL, username, app password (the graph's sync login by default); connecting is the connection test
- [ ] The password is stored in the OS keyring (`keyring` crate; Secret Service on Linux); only a reference is kept in device settings. Calendar settings are per device (not in the synced graph config), because credentials are per device

**Verify**
- [ ] A wrong password → a clear error message; correct → "Connected, N calendars found"
- [ ] `grep -r <password> ~/.config/notatnik ~/.local/share/notatnik <graph>` finds nothing
- [ ] The password is visible in "Passwords and Keys" (Seahorse) under the Notatnik entry

### P4.5 Calendar discovery and selection (CA-2)
- [x] Discover via the principal's `calendar-home-set` (falling back to `…/calendars/<user>/`) → list calendars with supported component types (VEVENT/VTODO)
- [x] Select: calendars to *read events* from (multi-select); one calendar to *write tasks* into (must support VTODO, or VEVENT if `tasks_as = vevent`)

**Verify**
- [ ] Against Docker Nextcloud: the list matches the calendars visible in the web UI, with correct VTODO support flags
- [ ] Selecting a VEVENT-only calendar for tasks with `tasks_as = vtodo` → validation error

## 4C: Events (read-only)

### P4.6 Pull events (CA-3)
- [x] Incremental pull (P4.11) of the calendars chosen for events; kept in the device cache (`calendar-events.json`, not the SQLite index: it is rebuilt from the server), recurring events expanded on demand for the days being shown (`event.rs`, with the `rrule` and `chrono-tz` crates: repeats, `EXDATE`, moved occurrences, time zones)
- [x] Journal page section "Calendar" (JR-5; today's journal, a day's own page and the older days of the feed); the timeline of a query shows the events too (QV-6)

**Verify**
- [ ] Create a single event, an all-day event, a weekly recurring event with one exception, and an event in another time zone in Nextcloud → each shows on the correct days and times in journals and the timeline
- [ ] Delete an event in Nextcloud → it disappears after the next sync
- [ ] Events are never written into Markdown files (`git status` on the graph shows no changes after a sync)

## 4D: Push tasks

### P4.7 Task → VTODO serialisation (CA-4)
- [x] Mapping as in the table above (`crates/caldav/src/map.rs`, `ical.rs`); deterministic output (the same block always produces byte-identical ICS, so pushes are idempotent). Times are floating (D45); `repeat:: … from-completion` has no iCalendar form and is written as the plain rule

**Verify**
- [ ] Table tests: ≥ 20 blocks (each state, date vs datetime, each priority, each repeat, blocks with links/tags/code in the title) → insta ICS snapshots
- [ ] Every generated ICS validates (parse it back with the `icalendar` crate; optionally run it through an external validator once)

### P4.8 Push engine (CA-4, CA-6)
- [x] Collect dated task blocks (excluding `cal:: no`); `EnsureBlockId` where missing (batched per file, guarded by the page hash) — manual "Send tasks to the calendar now" only; triggers are P4.11
- [x] Compare with the base state → PUT new or changed items with `If-None-Match: *` (new) or `If-Match: <etag>` (update) (`push.rs`; state in the device cache, `calendar-push.json`)

**Verify**
- [ ] Radicale: 10 new dated tasks → 10 items with the expected UIDs; a second sync → 0 requests (idempotent)
- [ ] Change one task's deadline → exactly 1 PUT
- [ ] Two app instances (two cache dirs, same graph folder) syncing alternately → still 10 items, no duplicates

## 4E: Pull task changes back

### P4.9 Remote → Markdown (CA-5)
- [x] For changed remote VTODOs with a notatnik UID: three-way merge per field (base / local / remote), producing `BlockOp`s applied via the normal guarded edit path (`pull.rs`, `apply_pulled` in `state/calendar.rs`; changed items are found by listing the calendar: the sync-token comes with P4.11). A time with another `TZID` is read as the clock time it states (no time zone database)
- [x] SUMMARY rule: write back only if the local first line is "plain" (no inline markup); otherwise log an entry (`title_kept`). Not done: a repeating task that a client moves on itself (new DUE, status unchanged) updates the dates but does not add a `completed::` date

**Verify** (fake server + Radicale)
- [ ] Remote completes a task → block becomes `DONE` with `done::`; the next push sends nothing
- [ ] Remote changes DUE → `deadline::` updated; other properties and children untouched (golden diff)
- [ ] Remote changes SUMMARY of a plain block → first line updated; of a block containing `[[link]]` → block untouched, a conflict entry in the log
- [ ] Local and remote change *different* fields → both changes kept
- [ ] Local and remote change the *same* field → resolved per CA-8, and logged

### P4.10 Externally created tasks
- [x] Import into the inbox page as described in the decisions; mapping fields as in the table. Off by default (`[calendar] import = "inbox"`, page `inbox = "inbox"`, or the switch in Settings → Calendar). Only open tasks are copied; a copy with a date is sent under its notatnik UID and the original is deleted after that; an undated copy is not sent, so its original stays and `PushState.imported` keeps it from being copied again

**Verify**
- [ ] Create a task in Nextcloud Tasks → after sync, it appears in `inbox` as a TODO block with an ID; the calendar contains it under the notatnik UID only (the original removed), so there are no duplicates
- [ ] With import disabled → external tasks are ignored and never deleted

## 4F: Incremental sync and scheduling

### P4.11 Incremental sync (CA-7)
- [x] Per calendar: `sync-token` (RFC 6578 `sync-collection` REPORT); fall back to a full ETag comparison when the token is invalid (server returns 403/409 `valid-sync-token`) (`pull::fetch`; the token is kept in `PushState` and only moves on when every change it covers was written; turning the inbox import on forgets it once)
- [x] Sync triggers: on app start, every N minutes (default 5), after local task changes (debounced 10 s), on manual "Sync now" (in the UI, `stores/calendarAuto.svelte.ts`: runs while the app is open; backoff 5, 10, 20, 30 min; a refused login is not retried by itself)

**Verify**
- [ ] With no changes, a sync run makes 1 request per calendar (the REPORT) and writes nothing
- [ ] An invalidated token (fake server) → full comparison succeeds, a new token is stored
- [ ] Network off → sync fails gracefully with a status message, retries with backoff; network back → succeeds

## 4G: Conflicts, deletion, logging

### P4.12 Conflict policy and sync log (CA-8)
- [x] Same-field conflicts: the most recent change wins (local: file mtime of the change, remote: `LAST-MODIFIED`/`DTSTAMP`); ties → local wins
- [x] Sync log view: time, item, direction, fields changed, conflicts with both values and a "use the other value" action

**Verify**
- [ ] Fake-server scenario table (≥ 10 rows: local newer / remote newer / tie, per field type) → expected winner
- [x] Each conflict appears in the sync log with both values; "use the other value" applies it and syncs

### P4.13 Deletion and un-dating (CA-9)
- [x] Block deleted, task un-dated, or `cal:: no` added → `on_delete` policy (`complete` default, or `delete`), subject to the deletion-safety rule
- [x] Remote deletion of a notatnik item → the block is **not** deleted; its dates are kept and a log entry plus `cal:: no` is suggested (never delete Markdown content because of a remote deletion)

**Verify**
- [x] Delete a dated task block → after 2 sync runs, the remote item is completed (default) or deleted (policy `delete`); after 1 run nothing happens
- [x] Simulate a page file temporarily missing (Nextcloud mid-sync: delete the file, sync, restore the file) → no remote item deleted or completed
- [x] Delete the item in Nextcloud → the block still exists, and the log shows the event

### P4.14 Compatibility matrix (manual)
Test each scenario with each client against your Docker Nextcloud, then against your real Nextcloud (test calendar):

| Scenario | Nextcloud Tasks (web) | Nextcloud Calendar (web) | Thunderbird | DAVx⁵ + Tasks.org (Android) | DAVx⁵ + jtx Board (Android) |
|---|---|---|---|---|---|
| Task appears with title, dates, priority | | | | | |
| Complete in client → DONE in Markdown | | | | | |
| Change due date in client → Markdown updated | | | | | |
| Repeating task: complete in client → next occurrence correct | | | | | |
| Create task in client → imported into inbox | | | | | |
| Link (URL) back to the app visible | | | | | |

**Verify**
- [ ] All cells green, or a documented workaround/limitation per red cell in `docs/phases/phase-4-compat.md`

---

## Manual acceptance checklist

- [ ] Enable sync with a dedicated test calendar on your real Nextcloud; use it for 3 days
- [ ] Switch to your real tasks calendar; use for two weeks; check the sync log daily for conflicts or surprises
- [ ] Complete tasks on your phone while the desktop is off; turn the desktop on → all changes arrive in the files
- [ ] Delete the cache folder (`~/.local/share/notatnik/graphs/<id>/`) → after a rebuild and full sync: no duplicates, no unwanted changes

Record: date / commit / notes:
