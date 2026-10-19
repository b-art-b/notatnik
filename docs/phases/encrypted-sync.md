# Encrypting chosen paths on a WebDAV server (E1 to E7)

**Goal:** some files of a graph are stored encrypted on the WebDAV server and the rest as they are. On the device
everything is plain Markdown and is edited, searched and synced the same way. Which files are encrypted is a list of
folders and files that you choose.

**Requirements covered:** EN-11 to EN-18 (and EN-2, EN-3, EN-4 as they exist).
**Depends on:** the WebDAV sync (SY-1 to SY-9, [webdav.md](../webdav.md)) and `notatnik-crypt` (P1).
**Design and decisions:** [encryption.md](../encryption.md) §6, D50.
**Status:** planned. Nothing here is built; start only when asked.

## Exit criteria

1. Every task checked, with every **Verify** item passing; `scripts/check.sh` passes.
2. A scan of everything the fake server holds, after sessions with marked paths, finds none of the marker strings
   that were written into marked pages, and finds every marker of unmarked pages.
3. Two real devices against your Nextcloud for a week: no lost edit, no plain copy of a marked page on the server
   (the files, not the versions made before the mark), no deleted file while the key was locked.

## Not in this phase

Hidden file names, an encrypted replica for a folder that another tool syncs (EN-7), encrypting local files, Android.
Pictures used by a marked page are only listed (E5); marking them is the person's choice.

## Tasks

### E1 Rules and matching (EN-11, EN-12)
- [ ] `[encryption]` in the graph config (`crates/graph/src/config.rs`): `paths` (list of relative paths, `/` at the end for a folder), `calendar` (`exclude` or `allow`, default `exclude`); unknown values are reported like other config warnings
- [ ] A pure matcher in `notatnik-sync` (no I/O): path -> encrypted or not; nothing under `.notatnik/`; a conflict copy is matched by the path of the file it came from; `..`, absolute paths and empty entries are refused
- [ ] The remote name of a path (`p` -> `p.enc`) and back; a local file ending in `.enc` is reported and not synced

**Verify**
- [ ] Table test of the matcher: a folder and its subfolders, a single file, a name that only starts like a folder (`pages/private-x.md` is not in `pages/private/`), upper and lower case, Unicode names, `.notatnik/` entries, conflict copies of a listed file and of a file in a listed folder
- [ ] Config round trip: the `[encryption]` table is written and read back byte-stable; bad entries give warnings, never a panic
- [ ] A local file `x.md.enc` is skipped and shows in the sync warnings

### E2 The graph vault (EN-13, EN-14)
- [ ] Make a vault for the graph when the first path is marked: password twice, recovery key shown once, must be confirmed (as EN-4); the descriptor is written to `.notatnik/vault.json`
- [ ] Unlock with the password or the recovery key, lock, change the password, new recovery key (`notatnik-crypt` already does the cryptography); keys in memory only
- [ ] The keyring switch: store the data key in the system keyring (`Keys` needs a way to be rebuilt from the stored bytes), unlock at graph open, delete the entry when the switch goes off or the graph is forgotten
- [ ] A second device that attaches the graph reads `.notatnik/vault.json` from the first sync and asks for the password or recovery key
- [ ] API commands and DTOs for all of it; bindings regenerated

**Verify**
- [ ] App tests: make, confirm, unlock with the password and with the recovery key, wrong secret, lock; the vault file in the graph holds no secret (scan for the data key bytes and the password)
- [ ] The keyring switch with a fake keyring (`Secrets`): on stores, a new session unlocks without a password, off removes it; a keyring that refuses leaves the switch off with a message
- [ ] A vault that is missing or unreadable while paths are marked: sync reports it and uploads **nothing** of the marked files, never plain in their place
- [ ] Second device: attach, see "locked", unlock, files appear

### E3 The encrypting remote and the engine (EN-12, EN-15, EN-16)
- [ ] `EncryptingRemote` around `WebDavRemote`: `put` encrypts a marked path to `p.enc` (context `p`), `get` decrypts, `list` maps `p.enc` to `p` and marks it encrypted, `delete` removes `p.enc`
- [ ] The engine takes a set of paths to leave alone (while locked, and for undecryptable files): no plan entry on either side, state kept, not counted by the mass-delete guard
- [ ] The state remembers how each server copy is stored; a different rule starts the migration (upload the new form, then delete the old one); a failure of the second step is retried
- [ ] A server file that fails to decrypt (wrong key ID, cut short, swapped with another path) is an error for that file in the report; the sync goes on with the others

**Verify**
- [ ] `crates/sync/tests`, fake server: a marked file is on the server as `p.enc` and a scan of every file the server holds finds no marker of its text; an unmarked file is stored as it is
- [ ] Two devices (own state, shared server): an edit goes both ways, a change on both sides merges from the last synced text or gives a conflict copy that is also encrypted; an unchanged file is not uploaded again at the next sync (no ciphertext churn)
- [ ] Locked: plain files sync, marked files neither upload nor download nor delete, however the other side changed; 11 or more marked files do not trip the mass-delete guard; unlock, then one sync catches up
- [ ] Marking a synced file: `p.enc` appears, `p` is deleted. Unmarking: the reverse. Stop the run between the two steps (fault injection in the fake): the next sync finishes it and no file is lost
- [ ] Two `.enc` files swapped on the server, one cut at a chunk boundary, one with another key ID: reported, nothing overwritten locally
- [ ] A rename of a marked page: the old `.enc` is deleted, the new one made

### E4 In the app (EN-15, EN-16, EN-17)
- [ ] The sync context uses the graph's keys; the sync status gets `encryption` (off, locked, unlocked), `encrypted_waiting` (files left alone) and the file errors of E3
- [ ] Marking and unmarking go through API commands that change `config.toml` with a minimal text edit (ST-3), say whether the paths were synced before, and start a sync
- [ ] The calendar sync leaves out tasks of marked pages when `calendar = "exclude"`; tasks sent earlier leave the calendar by the usual safe way
- [ ] The rename and move plan lists a warning when a page enters or leaves a marked path
- [ ] Backup: the status says when backups are plain while paths are marked
- [ ] The log (D49) says what happened at `info` (paths, never text): a path marked, the vault unlocked or locked, files left alone because of the lock

**Verify**
- [ ] App tests with the fake server and a fake keyring: end to end for the cases of E3 through `dispatch`
- [ ] Calendar test: a task on a marked page is not sent; a task sent before the mark is completed in the calendar, not deleted, and the block in the notes is untouched
- [ ] Rename tests: into, out of and inside a marked folder give the warning; elsewhere none
- [ ] The log of a full session holds no text of any page (scan for the markers)

### E5 The UI (EN-18)
- [ ] Settings → Sync: an Encryption section (off, locked, unlocked), set up (password twice, recovery key shown once), unlock, lock, the keyring switch, the list of marked paths with *Remove*, the count of files left alone
- [ ] The page menu: *Encrypt this page*, *Encrypt this page and the pages below*, *Stop encrypting*; the dialog that says what happens, including the warning about older plain copies on the server and the images the page uses
- [ ] A lock mark on encrypted pages (page title and the sidebar list); the sidebar sync state shows "locked"
- [ ] `docs/keymap.md` updated; strings written as the rest of Settings

**Verify**
- [ ] Browser tests against the dev server with the fake WebDAV server: set up, mark a page, the server file is `.enc`, lock, edit on the second device, unlock, it arrives; unmark; the lock mark follows
- [ ] The warning dialogs appear for a page that was synced before and for a rename across the boundary
- [ ] No page text appears in the dialogs' network traffic except the commands that must carry it (the marker scan of the dev server log)

### E6 Against a real Nextcloud (manual and `--ignored` tests)
- [ ] `crates/app/tests/nextcloud.rs`: mark, sync, read the raw `.enc` over WebDAV and check no marker; second device decrypts; locked sync; unmark
- [ ] Record what Nextcloud's versions and trash keep after a late mark, so the warning of E5 states facts
- [ ] Nextcloud's own client and the web UI do not break on `.enc` files

**Verify**
- [ ] The tests pass against the Docker instance of `dev/nextcloud/`
- [ ] Manual: the checklist below

### E7 Later (on hold)
Hidden file names with a manifest; an encrypted replica for a folder that another tool syncs (EN-7); marking the images of a page with it; Android with the key in the keystore (EN-10).

## Manual acceptance checklist

- [ ] On your Nextcloud, mark one folder; use it for a week on two devices; look at the folder in the Nextcloud web UI: only `.enc` files in the marked folder, everything else readable
- [ ] Lock the key, edit plain and marked pages on the other device, sync: the plain ones arrive, the marked ones wait; unlock: they arrive
- [ ] Lose the password, use the recovery key; change the password; the other device keeps working after it asks again
- [ ] Mark a page that was synced before: read the warning, then check the Nextcloud versions and trash of that file

Record: date / commit / notes:
