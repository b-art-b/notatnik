# Logging

The desktop app, the dev server and the CLI write a log to the terminal (stderr). Start the app from a terminal
(`cargo tauri dev`, or run the installed binary) to see it. The libraries use the `log` macros; the three binaries
start the logger with `notatnik_log::init` (D49).

```
12:30:05.123 INFO  app::state  opened graph /home/me/notes (gab12cd): 214 pages, index updated: 0 new, 2 changed, 0 gone, 41 ms
12:30:07.900 INFO  app::state::syncing  sync with https://cloud.example/remote.php/dav/files/me/notes: 2 sent, 1 received, 0 deleted on the server, 0 deleted here, 0 merged, 0 conflicts (812 ms)
12:31:07.411 WARN  app::state::syncing  sync with https://cloud.example/...: no network (connection refused)
```

## Choosing what is shown

`RUST_LOG` as usual. Without it, our crates log at `info` (the CLI at `warn`, because its commands print their own
results) and everything else at `warn`.

```bash
RUST_LOG=debug cargo tauri dev                          # everything, with every command
RUST_LOG=notatnik_sync=debug,notatnik_caldav=debug ...  # the requests to the servers
RUST_LOG=warn ...                                       # problems only
```

The crates: `notatnik_app`, `notatnik_graph`, `notatnik_index`, `notatnik_sync`, `notatnik_caldav`,
`notatnik_desktop`, `notatnik_dev_server`. The browser tests start the dev server with `RUST_LOG=warn`.

## Levels

| Level   | What                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `error` | A bug or a failure the user has to act on: a command that failed with `internal`, a rename that stopped half way, the keyring refusing a password, a sync that failed for a reason other than "offline", a last graph that could not be reopened, an error in the UI nobody caught (`ui`)                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `warn`  | Something is off but the app goes on: refused calls (`invalid`), a conflict copy waiting for a merge by hand, a sync conflict, a file that could not be synced, a page another editor reformatted, a sync that stopped for wanting to delete most files, going offline, a backup that failed, a calendar item that could not be marked or removed, duplicated block IDs, a `config.toml` that cannot be read                                                                                                                                                                                                                                                                                                |
| `info`  | Things that happened and that you would want to know afterwards: a graph opened (pages, index, time), a sync or calendar sync that did something, pages deleted or restored, a rename, a restored backup or page version or snapshot file (the folder, never the secret), a backup written, a server copy verified (counts only, no file names), a restore point, the calendar sync paused or resumed, the encryption of a graph set up, unlocked or locked, paths marked or unmarked, the names of encrypted files hidden or shown, a file stored again in the other form, a key taken from the keyring, every entry of the calendar sync log, a command slower than 250 ms, the app starting and quitting |
| `debug` | Every command with its time, files changed on disk, every file a sync sent or received, every request to a server (method, address, status, time), syncs that did nothing, answers like `not_found`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

A problem that repeats is logged once when it starts: a sync that fails every minute because there is no network
logs a warning the first time, then at `debug`, and `info` when it works again.

## Never logged

Text of notes, blocks or titles, arguments and results of commands, passwords, the `Authorization` header, request
and response bodies, query strings, web addresses pasted into notes (`link_title` logs nothing) the path and the name of a file of this computer opened in the viewer (`local_open` logs its size only) and the address, the path and the name of a document of the graph's Nextcloud (`remote_open` logs its file number, its size and whether the cache was used, and an error says what went wrong without the path) and the addresses of clipped pages (`clip_page` logs sizes and counts only; an error names the host and not the path; the HTTP library's own `debug` line, which is off unless `RUST_LOG` asks for it, does show the address it asks for). Page names, file paths and calendar names are logged (they are what you need to find a problem).

## Adding a log line

Use the `log` macros in a library crate (add `log = "0.4"`); never `println!`/`eprintln!` there. `notatnik-core` has
no logging. Ask: would the user want this line after the fact (`info`), is it a problem (`warn`/`error`), or is it
a trace for whoever debugs (`debug`)? Anything that runs at every key press or every render is `debug` at most.
