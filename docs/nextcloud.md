# Using a graph in a Nextcloud folder

Put the graph folder inside a folder that the Nextcloud desktop client syncs. Notatnik never talks to
Nextcloud for notes: it only reads and writes ordinary files (ST-1, NC-1).

## What gets synced, what doesn't

| Location | Synced? | Notes |
|---|---|---|
| `pages/`, `journals/`, `assets/` | yes | the notes |
| `.notatnik/config.toml`, `templates/`, `plugins/` | yes | graph settings shared between devices |
| `.notatnik/tmp/` | **no, exclude it** | atomic-write temp files (NC-2) |
| `~/.local/share/notatnik/…` | never | index, history, state: per device, outside the graph |

## Exclude the temp folder (NC-2)

Every save writes a temp file in `.notatnik/tmp/` and renames it over the page (so a reader never sees a
half-written file). The temp file exists for milliseconds, but if the app is killed in the middle a leftover
can stay until the next start, and the sync client should not upload it.

Add this line to the client's ignore list (Settings → General → *Edit ignored files*, or `sync-exclude.lst`):

```
*/.notatnik/tmp/*
```

The exact pattern syntax depends on the client version; check it against your client during spike S3
(phase 0) and correct this page. Tick "Sync hidden files" is **not** needed to exclude it, but is needed if
you want `.notatnik/config.toml` to sync.

## Conflicts

If two devices change the same file before syncing, Nextcloud keeps one version and writes the other as
`name (conflicted copy YYYY-MM-DD HHMMSS).md`. Notatnik does not index these copies as pages; `notatnik check`
lists them (phase 1) and the conflict merge in phase 5 helps to resolve them.
