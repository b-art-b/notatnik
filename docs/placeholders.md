# Placeholders

Words in double braces that the app replaces with a value when something is used: a template is applied, or a query
runs. They are never written to the file in their replaced form: the placeholder stays in the template or in the
query text.

## Reference

| Placeholder                                        | Where              | Becomes                                                                                                                                                                                                                                                                                                                                                                                                 |
| -------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `{{title}}`                                        | templates, queries | The last part of the page name: `Jan` for `people/Jan`.                                                                                                                                                                                                                                                                                                                                                 |
| `{{namespace}}`                                    | templates, queries | The part before it: `people` for `people/Jan`; empty for a page without a namespace.                                                                                                                                                                                                                                                                                                                    |
| `{{date}}`                                         | templates, queries | Today as `YYYY-MM-DD`. In a template used for a new journal day: that day.                                                                                                                                                                                                                                                                                                                              |
| `{{cursor}}`                                       | templates          | Removed; after `/template` inserts the blocks, editing continues there. Without it, editing continues at the end of the first inserted block. The first `{{cursor}}` counts. A new page made from a template ignores it.                                                                                                                                                                                |
| `{{type}}`, `{{prop.<key>}}`                       | templates, queries | The page's `type`, or another property of it (frontmatter, key case does not matter). A link comes as `[[Name]]`, a list joined with `, `, a date as `YYYY-MM-DD`; empty when the page lacks the property (in a query that is reported as an empty value). In a template applied to a page without a type, `{{type}}` is the template's own type. A new page that does not exist yet has no properties. |
| `{{date+7d}}`, `{{date-1w}}`                       | templates, queries | The day moved by days (`d`), weeks (`w`), months (`m`) or years (`y`); a month end is clamped (`2026-01-31` + `1m` is `2026-02-28`). Spaces around the sign and the number are fine.                                                                                                                                                                                                                    |
| `{{yesterday}}`, `{{tomorrow}}`                    | templates, queries | The day before and after `{{date}}`.                                                                                                                                                                                                                                                                                                                                                                    |
| `{{monday}}` … `{{sunday}}`                        | templates, queries | That day of the week of `{{date}}` (the week runs Monday to Sunday). `{{next monday}}` is the first such day after `{{date}}`, a week later when it is that day.                                                                                                                                                                                                                                        |
| `{{weekday}}`, `{{week}}`, `{{month}}`, `{{year}}` | templates, queries | `Wednesday`, `2026-W42` (ISO week), `2026-10`, `2026`.                                                                                                                                                                                                                                                                                                                                                  |
| `{{time}}`, `{{datetime}}`                         | templates          | The time of day now, `09:05`, and `2026-10-14 09:05`. Not known in queries.                                                                                                                                                                                                                                                                                                                             |
| `{{selection}}`                                    | `/template`        | The text of the block the command was typed in, besides the command: type a name, then `/template`, and the template can use it. Empty in a new page's template.                                                                                                                                                                                                                                        |
| `{{clipboard}}`                                    | `/template`        | The text on the clipboard when the template is inserted. The clipboard is read only for a template that has this placeholder; the browser or system may ask for permission, and without it the placeholder is empty.                                                                                                                                                                                    |
| `{{ask:Label}}`, `{{ask:Label\|default}}`          | `/template`        | A question: before the blocks are inserted a dialog asks for each distinct label once (the default is the starting text), Enter inserts, Esc cancels and inserts nothing. The answer is used wherever that label appears, and is text only: a `{{date}}` typed as an answer stays as typed. Where nobody can be asked (a new page made from a template) the default is used, else nothing.              |
| `{{page}}`                                         | queries            | A link to the page the query block is on (`[[people/Jan]]`). Also written `[[{{page}}]]`.                                                                                                                                                                                                                                                                                                               |

The empty link `[[]]` is still accepted in a query and means `{{page}}`.

## In templates

A template is `templates/<type>.md` in the graph (see keymap, "Templates for page types"). The placeholders may
stand anywhere in its properties and blocks:

```markdown
---
type: person
added: { { date } }
---

- Contact {{title}}
  - Phone
```

They are filled in for the page that the template is applied to, when:

- `type` is set on a page, and the template of that type adds its missing properties and, for an empty page, its
  blocks;
- a new page is created below a namespace that has a template, or a new journal day is created and a journal
  template is set (Settings → Templates);
- `/template` (the text typed in the block is `{{selection}}`; `{{clipboard}}` is what you copied) inserts a template's blocks, or the blocks saved with _Save as template…_ (`templates/snippets/`), into a block.

Only the properties and blocks are filled in. The template file itself is never changed.

## In queries

A query block works on every page that contains it without being edited:

````markdown
```query
type: page
prop.type: person
prop.partner: {{page}}
view: table
columns: title, prop.importance
```
````

On the page `part. Capgemini` this lists the people whose `partner` is `part. Capgemini`. The same block on `part. EY`
lists that partner's people.

- `{{page}}` works wherever a page may be written: `page:`, `links:`, `tag:`, `ns:` and `prop.<key>:`.
- `{{title}}` and `{{namespace}}` are plain text, for example `ns: {{namespace}}` for the pages beside this one.
- `{{type}}` and `{{prop.<key>}}` read the properties of the page the query is on: `prop.type: {{type}}` lists the
  pages of the same type, `prop.partner: {{prop.partner}}` those with the same partner.
- In the agenda and in a saved query (sidebar) no page is known: `{{page}}`, `{{title}}`, `{{namespace}}`, `{{type}}` and `{{prop.<key>}}` are
  reported as an error with their line and column, instead of silently matching nothing. `{{date}}` works
  everywhere.

## In the app

- **Settings → Templates → Placeholders** lists this table.
- In a query block, typing `{{` offers the query placeholders with a one-line description. They are also offered
  among the values of `prop.<key>:`, `tag:`, `ns:`, `page:` and `links:`.
- Templates are plain files; the placeholders of a template are not completed while you edit the file in another
  editor.

## Not supported

Other names (`{{today}}`, `{{user}}`, ...) are left as they are. Placeholders are not nested and have no arguments.
A literal `{{page}}` in a template or query cannot be escaped yet.
