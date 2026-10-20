# Hidden file and folder names, and re-encrypting a graph (H1 to H7)

**Goal:** the server (and a plain backup) learn nothing from the *names* of encrypted files either: no page titles,
no folder structure. And a graph that is already encrypted can be **re-encrypted with a new key** without ever
sending plain text, so a key that may have leaked can be retired.

**Requirements covered:** EN-25 (hidden names), EN-26 (re-encrypting).
**Depends on:** [encrypted-sync.md](encrypted-sync.md) (E1 to E9), `notatnik-crypt`.
**Status:** built and tested (fake server, browser, real Nextcloud 35 on a local test server).
**Design and decisions:** [encryption.md](../encryption.md) §6, D57.

## Design

- **Where a marked file lives.** With `[encryption] names = "hidden"` a marked file is not `pages/a.md.enc` but one
  object `.notatnik-enc/<aa>/<name>.enc`: `<name>` is `Keys::file_name(path)` (32 hex characters, a keyed hash of the
  path, so the same file keeps its name and nobody without the key can test a guess), `<aa>` its first two
  characters (so no folder grows huge). The folders of the graph do not exist on the server for these files.
- **The path is inside.** The object holds `path length | path | content`, encrypted. Whoever opens it knows the
  path, and must check that `file_name(path)` is the object's name (a swapped object is refused).
- **Finding what is new.** A name the device does not know (not in its sync state) is downloaded and opened to learn
  its path, which is what a new file needs anyway. While the key is locked, such objects are left alone like any
  encrypted file. A rename elsewhere is a new name plus a missing old one; a deletion is a missing name.
- **Changing form is one rule.** The sync compares *where the copy is* with *where it should be*
  (plain path, `<path>.enc`, or the hidden object). They differ after switching the setting, or after the key was
  replaced (names depend on the key): the file is written at the new place first, then the old copy is removed
  (never neither, as for EN-16). That one rule is also the migration and the re-encryption.
- **Re-encrypting.** *Re-encrypt with a new key…* (key unlocked) makes a new vault (new data key, new password and
  recovery key, confirmed as in EN-22). At the next sync every marked file is stored again from the plain local
  copy under its new place and the old object is deleted. Plain text never leaves; other devices are asked for the
  new password. Copies made earlier stay in Nextcloud's file versions and trash under the old key: they are
  readable by whoever has the old key, so empty them (the app says so). Turning encryption off and on again is not
  an option: the files would be on the server in plain text in between.
- **Backups.** A plain snapshot stores a marked file as `.notatnik-enc/<random>.enc`, sealed to the graph's vault
  with the path inside, instead of `<path>.enc` with the path in the clear. The name is random because writing needs
  no key.
- **What stays visible.** The marked paths in `.notatnik/config.toml` (so mark whole standard folders with
  *Encrypt everything…*, whose list names nothing private), the number and sizes of the files, their times, the
  names of unmarked files, and `.notatnik/`.

## Not in this phase

Hiding the list of marked paths; padding sizes; hiding the times; hiding the names of unmarked files.

## Tasks

### H1 Names and objects (EN-25)
- [x] `notatnik-crypt`: `Keys::encrypt_named(path, data)` / `decrypt_named(bytes)` (path inside, bound to the hidden-object context)
- [x] `notatnik-core::encrypted`: the hidden folder, `hidden_object(name)`, its parts; `.notatnik-enc/` is reserved locally
- [x] Config: `[encryption] names = "visible" | "hidden"` (default `visible`; a vault made from now on sets `hidden`)

**Verify**
- [x] Round trip; a changed byte, a wrong key and a wrong context are refused; the path survives any characters
- [x] Config round trip and a warning for an unknown value

### H2 The sync (EN-25)
- [x] `Found` knows where the copy is; `Io` reads, writes and deletes at that place; the engine plans a `Recode` when the place differs from the wanted one
- [x] The state keeps the object name of a hidden copy; unknown objects are opened to learn their path (key unlocked), left alone when locked
- [x] `verify` and the folder-ETag shortcut understand hidden objects

**Verify** (fake server)
- [x] After a session the server holds no path segment of a marked file; a second device gets every file
- [x] Rename and delete on one device arrive on the other; an object swapped for another is refused
- [x] Locked device: hidden objects are left alone and counted
- [x] Switching visible to hidden and back stores every marked file again and leaves no old copy

### H3 Re-encrypting (EN-26)
- [x] `encryption_rotate`: needs the unlocked key; makes the new vault like EN-22 (confirmation of the recovery key) and marks the stored copies to be stored again
- [x] The next sync stores them under the new key and deletes the old objects

**Verify**
- [x] After a rotation no object of the old key is left; the other device, with the new password, reads everything; with the old key nothing opens

### H4 The app and UI (EN-25, EN-26)
- [x] Status: `names`; command to switch; Settings → Sync → Encryption: *Hide file names* switch, *Re-encrypt with a new key…*, wording about versions and trash

### H5 Backups (EN-25)
- [x] Plain snapshots store marked files as hidden objects with the path inside; compare, page restore and unpacking read them; locked: refused

### H6 Docs and tests
- [x] requirements, encryption.md (§6, threat model), file-format.md §11, keymap, architecture D57, testing, logging

### H7 Real Nextcloud
- [x] Migration, a second device, a rename and a rotation against the test Nextcloud
