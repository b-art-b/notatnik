# Encryption

What is encrypted, how, and in which order it is built. Requirement IDs: EN-1 to EN-10 in
[requirements.md](requirements.md) §3. The formats are specified in [file-format.md](file-format.md) §11.

Status: **P0 spec, P1 crypto core and P2 encrypted backups are built; the sync parts (P3 to P7) are planned and
waiting for review.**

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

## 5. Planned: encrypted sync (EN-7 to EN-10, P3 to P7)

Not built. The shape, so the formats of P1 and P2 fit it:

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
| P3 | Sync engine and the folder remote, two-device tests | planned |
| P4 | Settings → Sync & encryption (enable, recovery key, unlock, status) | planned |
| P5 | Moving an existing graph out of the synced folder (wizard, warnings) | planned |
| P6 | WebDAV remote for Nextcloud | planned |
| P7 | Android | planned (Phase 8) |

## 6. Verification

- P1: round trip in every chunk shape; wrong password, recovery key, changed password; flipped bytes, truncation
  at a chunk boundary, extension, a file moved to another path; fixtures from an earlier build must keep opening
  (`fixtures/crypt/`); Unicode normalisation of passwords.
- P2: a scan of the backup folders for a marker string after a full session with encryption on; compare and
  restore through the API and the UI; `notatnik decrypt` against a snapshot; the locked state; conversion of old
  snapshots; no fallback to plaintext.
- Later: two simulated devices, interrupted and half-written files, conflicts, and a scan of the replica folder.
