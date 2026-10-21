# Encrypt everything, list the exceptions (S1 to S5)

**Goal:** a graph can be encrypted the other way round: *everything* is stored encrypted on the server and the person
lists what stays plain. Forgetting to mark something can then never leak it, and the server no longer sees a list of
private folders, only the (harmless) list of what is public.

**Requirements covered:** EN-27.
**Depends on:** [hidden-names.md](hidden-names.md) (H1 to H7), [encrypted-sync.md](encrypted-sync.md) (E1 to E9).
**Status:** built and tested (fake server, browser).
**Design and decisions:** [encryption.md](../encryption.md) §8, D58.

## Design

- **Two scopes.** `[encryption] scope = "marked"` (the default, and what every existing graph has) keeps the rules of
  `paths`. `scope = "all"` stores every file encrypted except those that `except` lists. `except` takes the same
  entries as `paths` (a folder ends in `/`, anything else is one file). The two lists do not interfere: while the
  scope is `all`, `paths` is not used but kept, so going back to `marked` restores the earlier choice.
- **Always plain.** Everything below `.notatnik/` (the vault and the settings must be readable before the key is
  known) and the reserved `.notatnik-enc/` folder. A conflict copy follows the file it came from, as before.
- **One predicate.** `EncryptedPaths` is already the single answer to "is this file stored encrypted", used by the sync,
  `verify`, backups, the calendar and the page lock mark. It gains `EncryptedPaths::everything_except(list)`; no
  caller changes how it asks. `EncryptionSection::rules()` builds the right one from the config, and every place
  that read `encryption.paths` for this purpose uses it.
- **Changing the scope is the Recode rule.** The sync already stores a file again when the place of its copy differs
  from the wanted one (H2). Switching to `all` moves every plain file to its encrypted place; switching back, or
  adding an exception, moves the file back to a plain copy. No new sync code. Both directions need the key unlocked
  (the old copies are looked up and removed by the same code that opens objects).
- **Marking in `all`.** `encryption_mark` edits the list that belongs to the scope: with `all`, *encrypted = true*
  removes an exception and *encrypted = false* adds one (with the same refusal when a folder above already decides).
  `encryption_scope{all}` switches; `encryption_scope_preview{all}` says what it means (files, plain copies the server
  already holds, dated tasks that leave the calendar).
- **Setting up.** The setup form offers *Everything, except what I list* (recommended) and *Only what I choose*. The
  choice is kept with the pending vault and applied on confirmation, together with hidden names. The API default stays
  `marked`.
- **Calendar.** With `calendar = "exclude"` the tasks of encrypted pages stay out of the calendar, so under `all` no
  task is sent unless its page is an exception or the person allows it. The settings row says so.

## Not in this phase

Hiding the list of exceptions; exceptions by pattern (`*.png`); a device with an existing local copy that syncs for
the first time pushes with the rules of its own (still default) settings before the server's `config.toml` has
arrived. Connect a new device to an empty folder, as the app's own flow does.

## Tasks

### S1 Rules (EN-27)
- [x] `EncryptedPaths::everything_except`, `is_all`; `.notatnik-enc/` never encrypted
- [x] Config `scope` and `except` (defaults, known keys, warnings for a bad value or entry); `EncryptionSection::rules()`
- [x] Every reader of `encryption.paths` for the rules uses `rules()`

**Verify**
- [x] Unit tests: exceptions (file, folder, conflict copy), the always-plain folders, bad entries ignored
- [x] Config round trip and warnings

### S2 The sync (EN-27)
- [x] Test on the fake server: a new file anywhere is stored encrypted, an exception stays plain, switching scope in both directions moves the files and loses none

**Verify**
- [x] `crates/sync/tests/hidden.rs` or a new `scope.rs`: scenarios above, with hidden and visible names

### S3 The app (EN-27)
- [x] `EncryptionStatus.scope` and `except`; `encryption_scope`, `encryption_scope_preview`; `encryption_mark` by scope; setup argument `scope`
- [x] Backups, calendar, rename warnings and `verify` use `rules()`

**Verify**
- [x] App test: marked to all to marked with the objects on the fake server, an exception added and removed, the status

### S4 The UI (EN-27)
- [x] Settings: setup choice; *Encrypt everything…* switches the scope; the list of exceptions with *Encrypt again* and a field to add one; *Only encrypt what I choose* switches back
- [x] Page dialog and lock mark follow the scope

**Verify**
- [x] Vitest for the rule; browser test: set up with everything, exception from a page, switch back

### S5 Docs
- [x] requirements EN-27, encryption.md §8, D58, file-format (config keys), keymap, TODO, testing
