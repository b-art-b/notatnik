# Encryption

What is encrypted, how, and in which order it is built. Requirement IDs: EN-1 to EN-10 in
[requirements.md](requirements.md) §3. The formats are specified in [file-format.md](file-format.md) §11.

Status: **P0 spec, P1 crypto core and P2 encrypted backups are built. Encrypting chosen paths on a WebDAV server
(§6, EN-11 to EN-18, tasks E1 to E6 in [phases/encrypted-sync.md](phases/encrypted-sync.md)) is built and tested
against the fake server and a real Nextcloud. The encrypted replica for a folder that another tool syncs (EN-7) is on
hold.**

## 1. Goal and threat model

Everything that leaves the device for a place someone else can read, a backup folder, a USB disk or a Nextcloud
server, can be ciphertext only. The server, the disk and anyone with access to them learn nothing about the
content, titles or structure of the notes.

The graph folder on the device stays plain Markdown (ST-1): external editors, the CLI, the file watcher and the
index keep working as they do now. Protecting a stolen laptop is a different goal (full-disk encryption does it,
and so does a filesystem such as gocryptfs); this feature does not try to replace it.

What a server or backup disk can still see: that encrypted data exists, how many files, how big they are and when
they change. A rename looks like a deleted and a new file.

What encryption cannot undo: plaintext that already left the device (old Nextcloud file versions and trash, old
plaintext backups on other disks) and old blocks on an SSD. The app says so where it matters.

## 2. Alternatives that were weighed

| Idea | Why not (now) |
|---|---|
| One encrypted vault file for the whole graph | One conflict loses everything; no per-file sync; Markdown stops being the source of truth |
| Encrypt the files inside the graph folder | Protects the laptop too, but external editors, the CLI and the index need the password and a RAM-only index; much more change for a goal that was not asked |
| Filesystem encryption (gocryptfs, Cryptomator, LUKS) | Works today without the app, but the password and the setup live outside it and do not cover the backups |
| Joplin's way: plaintext locally, encryption when data is sent | This is the model we follow. Joplin keeps a master key wrapped by the password, a key ID in every item header, several keys possible, and a password change that only re-encrypts the keys. See below for what we take and what we change |

From Joplin we take: keys wrapped by the password (a password change touches no data), a key ID in every file
header, chunked encryption for big files, names that reveal nothing, and honest wording about what is and is not
protected. We change: no "decrypt later in the background" state (the graph is unlocked or locked), a **recovery
key** instead of a reset that destroys the old data, and one marker in the encrypted folder itself so no device
has to be switched over by hand.

## 3. Keys (EN-2, EN-3, EN-4)

A vault holds two random secrets: the **data key** (32 bytes, encrypts files) and an **X25519 secret key** (opens
sealed backups). They are wrapped twice, once by a key derived from the **password** (Argon2id, 64 MiB, 3 passes,
a random salt per slot) and once by the **recovery key** (32 random bytes written as 14 groups of four letters
and digits, with a check so a typing mistake is reported as such). Either opens both secrets.

- The vault descriptor (JSON) is public: Argon2 settings, the wrapped secrets, the key ID and the public key.
  Without the password or the recovery key it opens nothing.
- Changing the password rewraps the secrets. No file is re-encrypted. A new recovery key can be made the same way;
  the old one then stops opening the current descriptor.
- Nothing secret is stored: not the password, not the keys. They live in memory while unlocked and are wiped when
  dropped. Backups are unlocked for a session and lock again after 15 minutes without use, or by hand.
- A lost password with a lost recovery key means lost data. There is no reset.

## 4. Encrypted backups (EN-5, EN-6, P2)

A snapshot is written as before (a zip), then **sealed to the vault's public key**: a random file key encrypts the
zip in chunks, and the file key is wrapped to the public key with a fresh ephemeral key pair. So the automatic
backups, the backup on quit and the copy in the second folder need no password. Reading one needs the password or
the recovery key. Every sealed file carries a copy of the vault descriptor, so **the file and the password are
enough to restore it**, on any machine, even with the app gone: `notatnik decrypt file.zip.enc`.

- Names: `notatnik-<graph id>-<time>[-before].zip.enc`. Retention, the second folder, the list and the
  comparison work as for plain snapshots; opening one (compare, restore, device settings) needs an unlocked vault.
- Turning encryption on never falls back to plaintext: if the vault is missing or unreadable the backup fails and
  says so. Existing plain snapshots can be converted (they are sealed, then the plain file is removed), in the
  backup folder and in the second folder.
- A snapshot made after a password change carries the new descriptor; older ones keep the descriptor they were
  made with, so the old password still opens them. The recovery key works for all of them unless it was replaced.
- Fully verifying a sealed file needs the private key. Writing checks the zip before sealing and the header
  after; with an unlocked vault the whole file is read back as well.

## 5. Earlier plan: an encrypted replica of the whole graph (EN-7, EN-10; on hold)

Not built, and replaced for now by §6, which encrypts only the paths you choose. The shape, so the formats of P1
and P2 fit it:

- The local graph moves out of any synced folder. An **encrypted replica** (one file per page and asset, names
  from `Keys::file_name`, the path inside the encrypted payload, a marker `vault.json` in the folder) is what
  Nextcloud, Syncthing or a USB disk holds.
- A sync engine keeps, per path, the last synced content and the remote ID or ETag, and decides push, pull or
  conflict. Editing on one device at a time (laptop, then phone) makes conflicts rare; a stale edit is never
  overwritten and goes through the existing conflict handling (ST-4, ST-6) and merge from the last synced copy.
- The engine talks to a **remote** with two implementations: a folder (another tool moves the files; the
  desktop, tests, Syncthing) and WebDAV (the app syncs itself with Nextcloud, using ETags and `If-Match`; the
  phone). Credentials go in the OS keyring like the CalDAV password (CA-1).
- Android (Phase 8) uses the same engine with background sync; the key may be kept in the Android keystore as an
  opt-in.

| Part | What | Status |
|---|---|---|
| P0 | This document, requirements, formats | done |
| P1 | `notatnik-crypt`: vault, slots, recovery key, file encryption, names, sealing | done |
| P2 | Encrypted backups, `notatnik decrypt`, Settings | done |
| P3 | Sync engine, two-device tests | built without encryption, as the WebDAV sync ([webdav.md](webdav.md)); the folder remote was dropped |
| P4 | Settings → Sync & encryption (enable, recovery key, unlock, status) | planned, as E5 of §6 |
| P5 | Moving an existing graph out of the synced folder (wizard, warnings) | on hold with the replica |
| P6 | WebDAV remote for Nextcloud | built, plain files; encrypting chosen paths is §6 |
| P7 | Android | planned (Phase 8) |

## 6. Encrypting chosen paths on a WebDAV server (EN-11 to EN-18)

Built. The idea in one sentence: **the graph on this device is plain, the server holds some files as they are
and some encrypted, and you choose which.** Pages of both kinds are edited, searched and synced the same way.

### 6.1 What you get

- A list of paths marks what is encrypted: a folder (`pages/private/`: it and everything below) or one file
  (`pages/Health.md`). Journals (`journals/`) and assets (`assets/private/`) are marked the same way. Everything
  else is stored plain, so the Nextcloud web UI, other apps and a phone still read it.
- A marked file `pages/private/Health.md` is stored on the server as `pages/private/Health.md.enc`. The device
  keeps `Health.md` as plain Markdown; the index, search, backlinks, queries and the agenda see it as before.
- Another device that has the password (or the recovery key) downloads the `.enc` file, decrypts it and shows it
  as an ordinary page; an edit there is encrypted on upload. Merging and conflicts work on the decrypted text,
  on the device, exactly as for plain files (SY-4).
- Only WebDAV. A folder that another tool syncs is not covered (EN-7, on hold); local files are not encrypted.

### 6.2 Decisions

| Topic | Decision | Why |
|---|---|---|
| Where the marks live | `[encryption] paths = ["pages/private/", "pages/Health.md"]` in `.notatnik/config.toml` (a graph setting that syncs, EN-11) | A page property fails when the page is moved; a path rule is what a person means by "this folder". The list is plain on the server: it shows which names are encrypted, which the file names show anyway |
| Where the work happens | In the sync engine, in one place (`crates/sync/src/forms.rs`): it lists the server by the path a file is the copy of (`p` or `p.enc`), reads and writes the form each file has or should have, and decrypts after `get` and encrypts before `put`. The engine keeps comparing local plaintext hashes and remote ETags | Merge, conflicts, the state file and the plan stay as they are. Re-encrypting unchanged text gives different bytes, but the engine uploads only when the local hash changed and treats a new ETag with equal decrypted content as "nothing to do" (it already did for Nextcloud's ETag changes). A wrapper around the remote could not see that a path exists in both forms, so the engine does the mapping |
| What is visible | File and folder names (with `.enc`), sizes, times, the number of files, and the list of marked paths. Not visible: content. Hiding names needs a manifest and is not part of this | Keeps the listing, the folder ETag shortcut (SY-3) and the migration simple. Name neutral titles for pages that matter |
| Which vault | One vault per graph, descriptor `.notatnik/vault.json` (public, synced as a plain file); separate from the backup vault, which belongs to the device (EN-13) | A second device needs the same keys; the backup vault is device-wide and not part of the graph. At setup the same password can be typed again, but the keys are different |
| The key | Unlocked per session, or kept in the system keyring (opt-in, EN-14). The keyring entry holds the data key, not the password, so starting the app needs no Argon2 run | The keyring is the same Secret Service the sync and calendar passwords use (SY-7). Anyone logged in to the desktop session can then read the key, which the settings text says |
| Locked | Marked files are left alone on both sides (EN-15). The engine computes a "leave these paths alone" set; their state entries stay, and the folder ETag shortcut is not saved while any file is left alone | Without it, a locked sync would see local files without a server partner and delete or upload them wrongly. The guard against mass deletion must not count them. Without the second rule an encrypted file that was left alone would stay hidden behind an unchanged folder ETag even after unlocking (found by a test) |
| Changing a mark | The state records how the server copy of each path is stored. When the rule differs, the engine uploads the new form, then deletes the old one; the second step is retried until it works (EN-16) | An interrupted run leaves both forms, never neither. The next run sees "both exist, rule says encrypted" and removes the plain one |
| The server's list wins | A server file `p.enc` is an encrypted copy of `p` whatever the rules say; the rules decide how a local file is *uploaded* | A new device reads the rules from `config.toml` in the same sync, and an unmarked path with an `.enc` copy is simply migrated back |
| `.enc` is reserved | A local file whose name ends in `.enc` is not synced and is reported | Otherwise it could not be told from an encrypted copy |
| Conflict copies | A "conflicted copy" is matched by the path of the file it came from | A copy of a marked single file would otherwise be uploaded plain |

### 6.2a Before the server exists (EN-19)

The vault, the key and the marks do not depend on a server: they are set up in Settings → Sync on a local graph and
marks are made with *Encrypt…* as usual. *Connect to a server…* (SY-9) keeps the key unlocked across the reopen of the
graph and the first sync uploads marked files as `.enc`, never plain. Doing it in this order is the only way to keep
the plain text out of the server's trash bin and file versions (see 6.3); marking after the upload cannot undo them.

### 6.2b Checking the server copy (EN-20)

`notatnik verify <graph>` (also `--url`, `--user`, `--json`) lists the server folder, downloads every file, decrypts
the `.enc` copies and compares each with the file in the graph folder by hash. Nothing is written on either side.
The server login comes from the device settings of the graph (or `--url`/`--user`) and its password from
`NOTATNIK_WEBDAV_PASSWORD`; the graph's password or recovery key comes from `NOTATNIK_PASSWORD`. Both are asked for
when the variables are not set. It reports: files that match (and how many of them were decrypted), files that
differ (an edit not synced yet, or damage), files only on one side, copies that cannot be decrypted, marked paths
that the server holds as plain text, encrypted copies that are not marked, and paths stored in both forms. The exit
code is 2 unless all of that is empty. To type the passwords without echo in zsh or bash use `read -rs` (without `-r` a backslash in a password is swallowed and the unlock fails with "wrong password"). Run it after a sync: a local edit made since then shows as "differs". `--no-key` skips the graph's password: encrypted copies are then only listed (as not checked), which is enough to see that none are left.

### 6.3 Roads for plain text that must be closed

1. **Calendar.** Tasks of a marked page would reach the CalDAV server in plain. They are left out of the calendar
   sync by default (`[encryption] calendar = "exclude"`; `"allow"` is the person's explicit choice). Tasks that were
   already sent leave the calendar through the normal safe path (completed, not deleted, `on_delete`).
2. **Old plain copies.** Marking a page that was already synced leaves its earlier text in Nextcloud's file
   versions and trash bin; plain WebDAV cannot remove them. The app says so before marking such a path and names
   what to do (empty the trash and delete the versions in Nextcloud). Mark before the first sync when you can.
3. **Backups.** A backup is a zip of the local, plain graph. With backup encryption off, marked pages are plain in
   the backup folder and the second folder. Settings → Backup warns while that is so.
4. **Moves.** Renaming or moving a page into or out of a marked path changes how it is stored. The rename dialog
   says so.
5. **Images.** A marked page's pictures stay plain unless their folder is marked too. The page's menu says which
   images it uses.
6. **Logs.** Nothing from a note is logged (D49); the log of the sync shows paths, not text.

### 6.4 Parts

| Part | What | Status |
|---|---|---|
| E1 | Rules in the config, matching, `.enc` rules | built |
| E2 | The graph vault: make, confirm, unlock, lock, keyring switch, join from a second device | built |
| E3 | The engine: forms, the "leave alone" set, the migration step | built |
| E4 | App: status fields, calendar exclusion, rename and backup warnings | built |
| E5 | UI: Settings → Sync → Encryption, the page's *Encrypt…* button, the lock mark, keymap | built |
| E6 | Real Nextcloud and two-device checks, manual checklist | tests built; manual checklist open |
| E7 | Later: hidden names, a replica for folder tools (EN-7), the images of a page | on hold |

The tasks and their Verify steps are in [phases/encrypted-sync.md](phases/encrypted-sync.md).

## 7. Verification

- P1: round trip in every chunk shape; wrong password, recovery key, changed password; flipped bytes, truncation
  at a chunk boundary, extension, a file moved to another path; fixtures from an earlier build must keep opening
  (`fixtures/crypt/`); Unicode normalisation of passwords.
- P2: a scan of the backup folders for a marker string after a full session with encryption on; compare and
  restore through the API and the UI; `notatnik decrypt` against a snapshot; the locked state; conversion of old
  snapshots; no fallback to plaintext.
- E1 to E5: a scan of everything the fake server holds for a marker string after a full session; two devices
  (own device folders, one server); locked and unlocked; a mark added and removed; interrupted migration; a
  server that swaps two `.enc` files or cuts one short; the calendar test with a marked page; browser tests of
  the pane and the page menu.
- EN-20: `notatnik verify` against the fake server: a clean graph, no key, an unsynced edit, a file on one side only, a flipped byte in an `.enc` file, a marked path stored as plain text (`crates/sync/tests/encrypted.rs`); the command with its exit codes and a wrong password (`crates/cli/tests/cli.rs`).
- E6: the same against a real Nextcloud (`crates/app/tests/nextcloud.rs`), including what its versions and trash
  keep.
