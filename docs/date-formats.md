# Date formats

Notes and files always store dates as `YYYY-MM-DD` (see [file-format.md](file-format.md)). How dates are
**shown** (journal headings, the agenda, `[[2026-10-03]]` links) is a per-device setting in
`~/.config/notatnik/settings.toml`, never synced and never written to the graph:

```toml
[ui]
date_locale = "pl-PL"          # language of month and weekday names (any BCP 47 tag)
date_format = "custom"         # full | long | short | iso | custom
date_pattern = "dddd, D MMMM YYYY"   # used only when date_format = "custom"
```

You can edit the file by hand or use **Settings → This device** (the preview updates as you type).

## Presets

For `2026-10-03`, language `en-GB`:

| `date_format`    | Result              | Notes                                                                           |
| ---------------- | ------------------- | ------------------------------------------------------------------------------- |
| `full` (default) | `Sat, 3 Oct 2026`   |                                                                                 |
| `long`           | `3 October 2026`    |                                                                                 |
| `short`          | `03/10/2026`        | field order follows the language (`en-US`: `10/03/2026`, `de-DE`: `03.10.2026`) |
| `iso`            | `2026-10-03`        |                                                                                 |
| `custom`         | from `date_pattern` | an empty pattern falls back to `full`                                           |

## Custom patterns

The pattern language is the common subset of [Moment.js / Day.js](https://day.js.org/docs/en/display/format)
tokens (the same letters you may know from those libraries). Only the tokens below exist; anything else
is not supported (no time, no week numbers).

| Token    | Meaning                    | Example (2026-03-07, `en-GB`) |
| -------- | -------------------------- | ----------------------------- |
| `YYYY`   | 4-digit year               | `2026`                        |
| `YY`     | 2-digit year               | `26`                          |
| `MMMM`   | month name, long           | `March` (`pl-PL`: `marca`)    |
| `MMM`    | month name, short          | `Mar`                         |
| `MM`     | month number, 2 digits     | `03`                          |
| `M`      | month number               | `3`                           |
| `DD`     | day, 2 digits              | `07`                          |
| `D`      | day                        | `7`                           |
| `dddd`   | weekday name, long         | `Saturday`                    |
| `ddd`    | weekday name, short        | `Sat`                         |
| `[text]` | literal text, copied as is | `[Week of]` → `Week of`       |

Rules:

- Month and weekday names come from `date_locale`. Polish uses the form that fits a date
  (`7 marca`, not `marzec`).
- Every other character (spaces, `.`, `/`, `-`, `,`, brackets are the exception) is copied.
- **Letters that are tokens are replaced even inside words.** Put plain text in brackets:
  `[Due:] D MMM` works, `Due: D MMM` does not (the `D` of `Due` becomes a number).
- Unknown letters are copied unchanged. The pattern can be at most 80 characters.

Examples:

| Pattern             | Result (`en-GB`)         |
| ------------------- | ------------------------ |
| `D.MM.YY`           | `7.03.26`                |
| `YYYY/MM/DD (ddd)`  | `2026/03/07 (Sat)`       |
| `dddd, D MMMM YYYY` | `Saturday, 7 March 2026` |
| `[Week of] D MMM`   | `Week of 7 Mar`          |

## Where it applies

Used for display only: journal headings and list titles, the agenda, rendered `[[date]]` links,
timeline labels. **Not affected:** the files, the date pickers (always `YYYY-MM-DD`), query clauses
(`deadline: < +7d`, `2026-10-05`) and the `/date` slash command output (`[[2026-10-03]]`).
