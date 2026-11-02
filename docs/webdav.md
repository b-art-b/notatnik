# Graphs on a WebDAV server (Nextcloud)

Status: **built**. Files are stored as they are; chosen paths can be stored encrypted (see below and [encryption.md](encryption.md) §6).
Requirements SY-1 to SY-8 in [requirements.md](requirements.md), decision D42 in [architecture.md](architecture.md).

## What it is

A graph can live in a folder on a WebDAV server instead of a folder of this computer. This device always keeps a
full copy in `~/.local/share/notatnik/replicas/<name>/` (the _replica_), so everything works without a
connection: search, the index, backups and the file watcher see an ordinary graph folder. The crate
`notatnik-sync` brings the replica and the server folder in step in the background.

Nothing else in the app knows about the server. The sync never touches the open graph's data structures, it only
reads and writes files, and the watcher picks up what it writes like any external change.

## Using it

1. Start screen (Ctrl+K, "Switch graph") → **New graph on a server…**.
2. Server (`https://cloud.example.org`, or the full address up to `…/remote.php/dav/files/<user>`), user name,
   password (on Nextcloud create an _app password_ in Settings → Security), folder (default `Notes/notatnik`).
3. **Check the folder** looks at it without changing anything. Then:
   - the folder is new or empty → **Create the graph** (the first sync uploads it);
   - the folder already holds a graph → **Download and open** (a second device, the phone);
   - the folder holds other things → refused; choose an empty or new folder.
4. The sidebar shows `☁ Synced 3 min ago`; click it to sync now. Settings → Sync shows the details, asks for the
   password again when needed, and has **Disconnect** (keeps the notes here as a local graph).

A graph that is a local folder can move to a server in place: Settings → Sync → **Connect to a server…**, the same
form. The folder must be empty or new (a folder with a graph or anything else is refused; two graphs are not merged).
**Upload this graph** keeps the pages where they are and uploads them with the first sync. To encrypt some of them,
set up encryption and mark them **before** this upload (Settings → Sync → Encryption works on a local graph too): they
then go up encrypted and the server never holds their text. Marked after the upload, the server's trash and versions
keep the plain copies (docs/encryption.md).

Plain `http://` is accepted only for this computer and the local network (`localhost`, `127.*`, `10.*`,
`192.168.*`, `172.16–31.*`, `*.local`); everything else needs `https://` (SY-2).

## What the server was found to do (probe of 2026-10-17, Nextcloud 35.0.2)

| Question                                                              | Answer                             | Used for                                      |
| --------------------------------------------------------------------- | ---------------------------------- | --------------------------------------------- |
| `PUT` with `If-None-Match: *` on an existing file                     | 412                                | creating a file never overwrites              |
| `PUT`/`DELETE` with a wrong `If-Match`                                | 412                                | an edit made elsewhere is never overwritten   |
| `PUT` response                                                        | `ETag` header, same as `PROPFIND`  | no extra request after an upload              |
| ETag of a folder when a file changes below it                         | changes, up to the account root    | skip unchanged folders (SY-3)                 |
| `MKCOL` of an existing folder / with a missing parent                 | 405 / 409                          | creating folders from the top                 |
| `PUT` into a missing folder                                           | 404                                | folders are made before files                 |
| `sync-collection` REPORT on files                                     | not supported (415)                | the sync walks folders by ETag instead        |
| hidden folders (`.notatnik/`), spaces, `(`, `#`, `?`, non-ASCII names | work when percent-encoded          | graph layout is stored as it is               |
| `X-OC-MTime` on `PUT`                                                 | accepted                           | files keep their modification time            |
| a 25 MB `PUT`                                                         | fine in one request                | pasted screenshots                            |
| names that differ only by case                                        | two files                          |                                               |
| HTTPS                                                                 | not available on the test instance | plain `http` allowed for local addresses only |

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

| Here    | There            | Since last sync | Action                                                         |
| ------- | ---------------- | --------------- | -------------------------------------------------------------- |
| changed | same             |                 | upload, only if the server still has the old ETag (`If-Match`) |
| same    | changed          |                 | download                                                       |
| changed | changed          |                 | download to compare: equal → nothing; different → **conflict** |
| deleted | same             |                 | delete on the server (`If-Match`)                              |
| deleted | changed          |                 | download (an edit beats a deletion)                            |
| same    | deleted          |                 | move here to `.notatnik/trash/`                                |
| changed | deleted          |                 | upload again (an edit beats a deletion)                        |
| new     | new (no history) |                 | compare contents; equal → nothing, different → conflict        |

When a **Markdown page** changed on both sides and the sync has the page as it was at the last sync (kept beside the
replica in `<replica>.sync-bases/synced/`, outside the graph), the two versions are **merged block by block**
(`notatnik_core::merge`, ST-6). A clean merge (no block changed on both sides) is written locally and uploaded
under `If-Match`; nothing is left over. The setting is _Settings → Sync → Merge changes from two devices_ (on by
default).

Otherwise a **conflict** keeps the server's version at the path and saves yours next to it as
`name (conflicted copy YYYY-MM-DD HHMMSS).ext`, and the common ancestor is kept for that copy in
`<replica>.sync-bases/ancestor/`. The copy is uploaded too, so every device sees it, and the conflict view lists
it. There you decide each block that both sides changed (page, copy, both or your own text), see the merged page
as a diff, and write it; the copy then goes to `.notatnik/trash/`. Without a kept ancestor (another device made
the copy, or the Nextcloud client did) the ancestor is guessed from the page's version history; with none the
page can only be kept or replaced by the copy.

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

**Android** (SY-10, [phases/android-sync.md](phases/android-sync.md)): the passwords are kept with the Android
Keystore. One AES-256-GCM key, which cannot leave the device, encrypts them; the encrypted values are in a private
preferences file of the app, each bound to its own name. `SecretsPlugin.kt` (in `src-tauri/gen/android`) does this and
`src-tauri/src/keystore.rs` connects it to the same `Secrets` seam as the desktop keyring. A value that cannot be
decrypted (a restore onto another phone, a wiped Keystore) counts as no password: the sidebar says _Sign in needed_ and
Settings → Sync asks for it again. Disconnecting deletes the value.

To try the app against the test Nextcloud from the emulator, use `http://10.0.2.2:<port>` (plain `http` is allowed for
the local network) and a server that trusts that host name: `dev/nextcloud/setup.sh` adds it to `trusted_domains`;
otherwise Nextcloud answers 400 and the connection test says so.

## Encrypted paths

A file under a path listed in `[encryption] paths` of `.notatnik/config.toml` is stored as `<path>.enc` (EN-11,
EN-12); the replica on this device stays plain. The engine lists the server by the path a file is the copy of, so
`p.enc` is the server's copy of `p`, and the plan, the state file, merging and conflicts work on the plain text as
before. What differs:

- A file whose server form differs from what the rules ask for is stored again in the right form, then the old copy
  is deleted; both forms side by side (an interrupted run) are repaired by the next sync, and only when their
  contents are equal. Different contents are reported and nothing is deleted.
- While the key is locked, marked files and server files that end in `.enc` are left alone, and the folder ETag
  shortcut is not used at the next sync.
- `notatnik verify <graph>` reads the whole server folder back, decrypts the `.enc` copies and compares them with the graph folder; it changes nothing (EN-20, docs/encryption.md §6.2b). `--no-key` only lists the encrypted copies.
- A copy that fails to decrypt (wrong key, cut short, swapped with another path) is an error of that file; the sync
  goes on with the others.
- What the server learns: names, folders, sizes and times of encrypted files, and that they exist. What stays on it
  after a file was synced in plain before it was marked: Nextcloud's file versions and trash bin (checked on a real
  server: the trash bin keeps the plain text).

## When another editor changes the files

A page that another editor saves on the server may come back with its lines joined: Nextcloud's web _Text_ editor
turns `- TODO call ^abc123` + `  deadline:: 2026-10-21` into one line. The sync reports such a page after it is
downloaded (the sidebar and Settings → Sync, SY-9); restore the earlier version from the page's History. Notatnik's
own files should not be opened in that editor.

## Limits

- The server and everything with access to it sees the files that are not marked as encrypted in plain text.
  Marked files are encrypted, but their names and sizes are visible (EN-11 to EN-18).
- One server folder per graph, one graph open at a time; the folder is typed or picked in the folder browser (Browse… next to the field: click through the server's folders, _New folder here_, _Use …_). The browser only looks; folders that do not exist yet, also several levels of them, are made when the graph is created.
- Files are sent one after another, whole; a broken upload starts again. A Nextcloud with a slow disk needs about
  half a second per file.
- Links _to_ a moved page written as relative Markdown paths in other pages are not rewritten (page links
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
