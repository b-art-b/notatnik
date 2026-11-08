# Choosing from a list in a template (TC1 to TC3)

**Goal:** a template can ask a question whose answer is **one of a fixed list**: `{{askchoice:Status|Open|Doing|Done}}`.
The dialog that already asks `{{ask:…}}` and `{{askpage:…}}` shows the choices; the chosen text goes where the
placeholder stands.

**Requirements covered:** TC-1 to TC-6 (see `requirements.md`); builds on the `{{ask:…}}` / `{{askpage:…}}` questions
(`docs/placeholders.md`, `crates/app/src/templates.rs`, `TemplateAskDialog.svelte`).
**Depends on:** `template_blocks` (it already returns the questions and takes the answers), the fill order of
`docs/placeholders.md`.
**Status:** TC1 to TC3 done and tested (Chromium and WebKit pending in the check).

## Decisions

- **Syntax:** `{{askchoice:Label|first|second|third}}`. The label is what the dialog shows; every part after the first
  `|` is a choice, split on `|` only, so choices may hold commas, colons and spaces (`10:30`, `Done, archived`). A
  choice cannot hold a literal `|`. Spaces around a choice are trimmed; empty choices are dropped. A placeholder with
  a label and no choice is an ordinary free-text question (`{{ask:Label}}`).
- **The first choice is the default.** It is preselected in the dialog, and it is the answer where nobody can be asked (a
  new page's or a journal day's template), exactly as `ask` uses its default there. No `*default` marker in this
  phase.
- **The answer is plain text,** like `ask`: the chosen text only; a `{{date}}` among the choices stays as typed. A choice
  may be a link (`[[Acme]]`), which then is a link in the block; no separate "page choice".
- **One question per label.** A label used twice is asked once and answers every place that uses it. If the same label
  is used with different kinds (`ask`, `askpage`, `askchoice`) the first one decides how it is asked, as it does
  now for `ask` and `askpage`.
- **The dialog.** Up to five choices: a radio group (↑/↓ move, the first is selected, Enter inserts). More than five: a
  filter field with the list under it (type to narrow, ↑/↓ and Enter), like the template picker. Esc cancels and
  inserts nothing. The choice is reachable by keyboard only and by touch (44 px rows).
- **No free text** in a choice question. To allow any value use `{{ask:Label|default}}`.
- **The file format does not change:** a template is a page with the placeholder as text.

## Tasks

### TC1 The question carries its choices

- `parse_ask` knows `askchoice:`; `Ask` and the DTO `TemplateAsk` get `choices: Vec<String>` (empty for the other kinds);
  `default` is the first choice; the answer replaces the placeholder with the chosen text, an answer that is not among
  the choices is used as given by the caller (the dialog never sends one). Bindings regenerated.
- **Verify:**
  - [x] unit tests: the choices of `{{askchoice:S|Open|Doing|Done}}` in order; trimmed; empty ones dropped; a label with no
        choice is a free-text question; colons, commas and spaces inside a choice kept; case-insensitive name; the same
        label twice asked once; `ask` + `askchoice` with one label asked once, by the first
  - [x] `template_blocks` through `dispatch`: the reply lists the question with its choices and default; with an answer
        the block text has it; with none (no dialog possible) the first choice is used
  - [x] a page's or a journal day's template with `askchoice` starts the page with the first choice and no dialog

### TC2 The dialog

- `TemplateAskDialog.svelte` shows a radio group or a filtered list for a choice question; the answer sent is the choice.
- **Verify:**
  - [x] with 3 choices: the first is selected, ↑/↓ move, Enter inserts the chosen one; Esc inserts nothing and the block goes on
        being edited
  - [x] with 8 choices: typing narrows the list, ↑/↓ and Enter pick; a filter with no match inserts nothing and says so
  - [x] a template with `ask` and `askchoice` shows one dialog with both fields in the order of the template, Tab moves
        between them, Enter inserts (`askpage` together with a choice not tested separately)
  - [x] by touch (rows 44 px); each field has the label as its accessible name (radiogroup / combobox)

### TC3 Tests and documents

- Playwright (Chromium and WebKit); `placeholders.md` (the new row and the no-dialog rule), `requirements.md` (TC rows),
  `testing.md`, `TODO.md`, `CLAUDE.md` (the doc list).
- **Verify:**
  - [x] the whole path: write a template with `askchoice`, `/template` on a block, pick the second choice by keyboard,
        the file holds it, undo removes the inserted blocks
  - [ ] `scripts/check.sh` and `scripts/check-webkit.sh` pass
