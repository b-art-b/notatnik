# Graphs on a WebDAV server (Nextcloud)

Status: **built** (plain files; encryption on the server is not built, see [encryption.md](encryption.md)).
Requirements SY-1 to SY-8 in [requirements.md](requirements.md), decision D42 in [architecture.md](architecture.md).

## What it is

A graph can live in a folder on a WebDAV server instead of a folder of this computer. This device always keeps a
full copy in `~/.local/share/notatnik/replicas/<name>/` (the *replica*), so everything works without a
connection: search, the index, backups and the file watcher see an ordinary graph folder. The crate
`notatnik-sync` brings the replica and the server folder in step in the background.

Nothing else in the app knows about the server. The sync never touches the open graph's data structures, it only
reads and writes files, and the watcher picks up what it writes like any external change.

## Using it

1. Start screen (Ctrl+K, "Switch graph") → **New graph on a server…**.
2. Server (`https://cloud.example.org`, or the full address up to `…/remote.php/dav/files/<user>`), user name,
   password (on Nextcloud create an *app password* in Settings → Security), folder (default `Notes/notatnik`).
3. **Check the folder** looks at it without changing anything. Then:
   - the folder is new or empty → **Create the graph** (the first sync uploads it);
   - the folder already holds a graph → **Download and open** (a second device, the phone);
   - the folder holds other things → refused; choose an empty or new folder.
4. The sidebar shows `☁ Synced 3 min ago`; click it to sync now. Settings → Sync shows the details, asks for the
   password again when needed, and has **Disconnect** (keeps the notes here as a local graph).

Plain `http://` is accepted only for this computer and the local network (`localhost`, `127.*`, `10.*`,
`192.168.*`, `172.16–31.*`, `*.local`); everything else needs `https://` (SY-2).

## What the server was found to do (probe of 2026-10-17, Nextcloud 35.0.2)

| Question | Answer | Used for |
|---|---|---|
| `PUT` with `If-None-Match: *` on an existing file | 412 | creating a file never overwrites |
| `PUT`/`DELETE` with a wrong `If-Match` | 412 | an edit made elsewhere is never overwritten |
| `PUT` response | `ETag` header, same as `PROPFIND` | no extra request after an upload |
| ETag of a folder when a file changes below it | changes, up to the account root | skip unchanged folders (SY-3) |
| `MKCOL` of an existing folder / with a missing parent | 405 / 409 | creating folders from the top |
| `PUT` into a missing folder | 404 | folders are made before files |
| `sync-collection` REPORT on files | not supported (415) | the sync walks folders by ETag instead |
| hidden folders (`.notatnik/`), spaces, `(`, `#`, `?`, non-ASCII names | work when percent-encoded | graph layout is stored as it is |
| `X-OC-MTime` on `PUT` | accepted | files keep their modification time |
| a 25 MB `PUT` | fine in one request | pasted screenshots |
| names that differ only by case | two files | |
| HTTPS | not available on the test instance | plain `http` allowed for local addresses only |

Folder ETags are an extension of Nextcloud, not part of WebDAV. The connection test writes a small file and checks
whether the folder's ETag changed; only then does the sync skip folders (`trust_folder_etags`, stored with the
graph). A server that does not do this is listed completely at every sync. A server that ignores `If-Match` or
`If-None-Match` is refused: syncing with it could lose edits.

The Notes app of Nextcloud shows the `.md` files below `Notes/` as notes (category = folder, title = file name).
A graph in `Notes/<name>` therefore appears there, readable. Do not edit those notes in the Notes app: it renames
a file when its first line changes, which would break page names.

## How the sync decides (SY-3 to SY-5)

Per file path it knows three things: the local content hash, the server's ETag, and what both were at the last
sync (kept in `~/.local/share/notatnik/replicas/<name>.sync-state.json`, outside the graph folder).

| Here | There | Since last sync | Action |
|---|---|---|---|
| changed | same | | upload, only if the server still has the old ETag (`If-Match`) |
| same | changed | | download |
| changed | changed | | download to compare: equal → nothing; different → **conflict** |
| deleted | same | | delete on the server (`If-Match`) |
| deleted | changed | | download (an edit beats a deletion) |
| same | deleted | | move here to `.notatnik/trash/` |
| changed | deleted | | upload again (an edit beats a deletion) |
| new | new (no history) | | compare contents; equal → nothing, different → conflict |

A **conflict** keeps the server's version at the path and saves yours next to it as
`name (conflicted copy YYYY-MM-DD HHMMSS).ext`. That copy is uploaded too, so every device sees it, and the app's
conflict view (ST-6) lists it. The same page edited on two devices at once is rare for one person; the cost of
the simple rule is that you merge by hand.

Not synced: `.notatnik/tmp/`, `.notatnik/trash/`, `*.partial`, `*~`, `.DS_Store`, symbolic links, names that are
not valid UTF-8.

**Mass deletion guard.** A sync that would delete more than ten files and more than half of the known ones stops
with an error and asks (Settings → Sync → "Delete N files and sync"). An emptied folder or a wiped server must not
spread to the other side.

**When it runs** (SY-6): about two seconds after the graph opens, five seconds after the last change on this
device, every minute, when the app closes (limited to 30 seconds together with the quit backup), and on
"Sync now". One sync at a time. Network errors stop the sync and are retried at the next round; a wrong password
stops it until a new one is entered.

## Passwords (SY-7)

The password is kept in the system keyring (Secret Service: GNOME Keyring, KWallet) under `notatnik`, keyed by user
and folder address. It is not in `settings.toml`, so it is not in backups either. Without a keyring the password
cannot be stored and the graph cannot be created; the error says so. The browser test build keeps passwords in
memory.

## Limits

- The server and everything with access to it sees the notes in plain text. Encryption on the server (EN-7 to
  EN-10) is planned and not built.
- One server folder per graph, one graph open at a time; no browsing of the server's folders (type the folder).
- Files are sent one after another, whole; a broken upload starts again. A Nextcloud with a slow disk needs about
  half a second per file.
- Links *to* a moved page written as relative Markdown paths in other pages are not rewritten (page links
  `[[…]]` are).
- Attaching works for folders made by Notatnik. Plain notes already in `Notes/` are not imported as pages.
- Android (Phase 8) will use the same engine with background sync.

## Testing

- `crates/sync/tests/sync.rs`: the engine against an in-memory WebDAV server (`fakedav`: Nextcloud-style ETags,
  conditions, mount path, a request log, switches to misbehave): first sync both ways, edits, conflicts, deletes,
  the guard, skipping folders, the connection test, login and server errors, the address rules, and a whole story of
  laptop and phone taking turns.
- `crates/app/tests/app.rs`: creating, attaching and disconnecting through the API.
- `ui/tests/webdav.spec.ts`: the start-screen form, the status line and Settings → Sync in the browser.
- Against a real server (not run by `scripts/check.sh`):
  `NOTATNIK_WEBDAV_URL=http://localhost:8080/remote.php/dav/files/me NOTATNIK_WEBDAV_USER=me NOTATNIK_WEBDAV_PASSWORD=… cargo test -p notatnik-sync --test sync -- --ignored`
  and `NOTATNIK_WEBDAV_SERVER=http://localhost:8080 … cargo test -p notatnik-app --test app -- --ignored`. Both
  create a folder `Notes/notatnik-live-…` and remove it.
