# Placeholders

Words in double braces that the app replaces with a value when something is used: a template is applied, or a query
runs. They are never written to the file in their replaced form: the placeholder stays in the template or in the
query text.

## Reference

| Placeholder | Where | Becomes |
|---|---|---|
| `{{title}}` | templates, queries | The last part of the page name: `Jan` for `people/Jan`. |
| `{{namespace}}` | templates, queries | The part before it: `people` for `people/Jan`; empty for a page without a namespace. |
| `{{date}}` | templates, queries | Today as `YYYY-MM-DD`. In a template used for a new journal day: that day. |
| `{{cursor}}` | templates | Removed. Reserved for placing the cursor. |
| `{{page}}` | queries | A link to the page the query block is on (`[[people/Jan]]`). Also written `[[{{page}}]]`. |

The empty link `[[]]` is still accepted in a query and means `{{page}}`.

## In templates

A template is `templates/<type>.md` in the graph (see keymap, "Templates for page types"). The placeholders may
stand anywhere in its properties and blocks:

```markdown
---
type: person
added: {{date}}
---
- Contact {{title}}
  - Phone
```

They are filled in for the page that the template is applied to, when:

- `type` is set on a page, and the template of that type adds its missing properties and, for an empty page, its
  blocks;
- a new page is created below a namespace that has a template, or a new journal day is created and a journal
  template is set (Settings → Templates);
- `/template` inserts a template's blocks into a block.

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
- In the agenda and in a saved query (sidebar) no page is known: `{{page}}`, `{{title}}` and `{{namespace}}` are
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
