<script lang="ts">
  import type { ResultRow } from '../../api'
  import { pageHash } from '../../route'
  import { setProperty } from '../../commands/exec'
  import { getDoc } from '../../stores/pages.svelte'
  import Nodes from '../Nodes.svelte'
  import Block from '../Block.svelte'
  import { shownDate } from '../../stores/format.svelte'
  import { app } from '../../stores/app.svelte'
  import { cellValue, sortRows } from '../../views'

  let {
    rows,
    columns,
    id,
    groupBy = null,
  }: { rows: ResultRow[]; columns: string[]; id: string; groupBy?: string | null } = $props()
  let sortCol = $state<string | null>(null)
  let desc = $state(false)
  let collapsed = $state<Record<string, boolean>>({})
  let editing = $state<{ row: string; col: string } | null>(null)

  // a journal page is named by its day: show it in the user's date format, like the list view does
  const pageLabel = (n: string) => (/^\d{4}-\d{2}-\d{2}$/.test(n) ? shownDate(n) : n)
  const cell = cellValue
  // A table of tasks (it has a `task` column): the state is its own column, so the text cell is the text alone, and a
  // date or priority column nobody filled in is left out instead of showing an empty strip.
  const taskTable = $derived(columns.includes('task'))
  const AUTO_HIDE = ['priority', 'deadline', 'scheduled', 'prop.created']
  const shown = $derived(
    taskTable
      ? columns.filter((c) => !AUTO_HIDE.includes(c) || rows.some((r) => cell(r, c) !== ''))
      : columns,
  )
  const LABELS: Record<string, string> = { task: 'State', 'prop.created': 'Created' }
  const label = (c: string) => LABELS[c] ?? c.replace(/^prop\./, '')
  const DATES = ['deadline', 'scheduled', 'prop.created']
  const ISO = /^(\d{4}-\d{2}-\d{2})(.*)$/
  // a date in the device's format, a time of day after it as written (`2026-10-14 09:00-10:30`)
  const dayLabel = (v: string) => {
    const m = ISO.exec(v)
    return m ? shownDate(m[1]) + m[2] : v
  }
  const late = (r: ResultRow, c: string) =>
    c === 'deadline' &&
    !!r.deadline &&
    r.deadline.slice(0, 10) < app.today &&
    r.task_state !== 'DONE' &&
    r.task_state !== 'CANCELED'
  // With `group:` the rows sit under one header row per group (labels come from the backend, in its order);
  // the header-click sort then orders the rows inside each group. Without it: one unlabelled group.
  const groups = $derived.by(() => {
    const m = new Map<string | null, ResultRow[]>()
    for (const r of rows) {
      const key = groupBy ? (r.group ?? '(none)') : null
      m.set(key, [...(m.get(key) ?? []), r])
    }
    return [...m.entries()].map(([label, items]) => ({
      label,
      link: items.some((r) => r.group_link),
      items: sortRows(items, sortCol, desc),
    }))
  })

  function header(col: string) {
    if (sortCol === col) desc = !desc
    else {
      sortCol = col
      desc = false
    }
  }
  async function save(r: ResultRow, col: string, value: string) {
    editing = null
    if (!r.block || !col.startsWith('prop.')) return
    const key = col.slice(5)
    await setProperty(getDoc(r.page), r.block.path, key, value.trim() === '' ? null : value.trim())
  }
</script>

<div class="wrap">
  <table class="table" class:tasks={taskTable} data-testid="query-table">
    <thead>
      <tr>
        {#each shown as c (c)}
          <th
            scope="col"
            class:text={c === 'text'}
            class="col-{c.replace(/[^a-z-]/g, '-')}"
            aria-sort={sortCol === c ? (desc ? 'descending' : 'ascending') : 'none'}
          >
            <button onclick={() => header(c)}>{label(c)}{sortCol === c ? (desc ? ' ▼' : ' ▲') : ''}</button>
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each groups as g (g.label)}
        {#if g.label !== null}
          <tr class="group" data-group={g.label}>
            <th scope="rowgroup" colspan={shown.length}>
              <button
                class="tog"
                aria-expanded={!collapsed[g.label]}
                aria-label={`Toggle ${g.label}`}
                onclick={() => (collapsed[g.label!] = !collapsed[g.label!])}
                >{collapsed[g.label] ? '▸' : '▾'}</button
              >
              <!-- the name opens its page for `group: page` and for a property value written as [[link]]; plain text stays text -->
              {#if g.link || groupBy === 'page'}
                <a href={pageHash(g.label)}>{pageLabel(g.label)}</a>
              {:else}{g.label}{/if}
              <span class="n">{g.items.length}</span>
            </th>
          </tr>
        {/if}
        {#each collapsed[g.label ?? ''] ? [] : g.items as r (r.id)}
          <tr data-row={r.id}>
            {#each shown as c (c)}
              <td
                class:text={c === 'text'}
                class:late={late(r, c)}
                class="col-{c.replace(/[^a-z-]/g, '-')}"
                title={c === 'text' ? undefined : cell(r, c)}
                ondblclick={() => c.startsWith('prop.') && (editing = { row: r.id, col: c })}
              >
                {#if editing && editing.row === r.id && editing.col === c}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    autofocus
                    aria-label={`Edit ${c}`}
                    value={r.link_props.includes(c.slice(5)) ? `[[${cell(r, c)}]]` : cell(r, c)}
                    onkeydown={(e) => {
                      if (e.key === 'Enter') void save(r, c, (e.currentTarget as HTMLInputElement).value)
                      if (e.key === 'Escape') editing = null
                    }}
                    onblur={(e) => void save(r, c, e.currentTarget.value)}
                  />
                {:else if c === 'linked-from'}
                  <!-- the pages that link to this row's page, journal days among them -->
                  {#each r.linked_from as from, i (from)}{i ? ', ' : ''}<a href={pageHash(from)}
                      >{pageLabel(from)}</a
                    >{/each}
                {:else if c.startsWith('prop.') && r.link_props.includes(c.slice(5)) && cell(r, c)}
                  <!-- a property written as [[Page]] is a page here too -->
                  <a href={pageHash(cell(r, c))}>{pageLabel(cell(r, c))}</a>
                {:else if c === 'page'}
                  <a href={pageHash(r.page)}>{pageLabel(r.page)}</a>
                {:else if c === 'task' && r.block?.task}
                  {@const t = r.block.task}
                  <span
                    class="state"
                    style:--c={`var(--task-${t.state.toLowerCase()}, var(--muted))`}
                    data-task={t.state}>{t.checkbox ? (t.state === 'DONE' ? '☑' : '☐') : t.state}</span
                  >
                {:else if c === 'priority' && cell(r, c)}
                  <span class="prio-chip" data-prio={cell(r, c)}>{cell(r, c)}</span>
                {:else if DATES.includes(c) && cell(r, c)}
                  <time>{dayLabel(cell(r, c))}</time>{late(r, c) ? ' ⚠' : ''}
                {:else if c === 'text' && r.block}
                  <!-- the list view shows a result as a full Block (state badge and all); a cell shows the text, so a task
                   gets a read-only badge here (change the state in the list view or on its page) -->
                  {#if r.block.task && !taskTable}
                    {@const st = r.block.task.state.toLowerCase()}
                    <span
                      class="state"
                      style:--c={`var(--task-${st}, var(--muted))`}
                      data-task={r.block.task.state}
                      >{r.block.task.checkbox
                        ? r.block.task.state === 'DONE'
                          ? '☑'
                          : '☐'
                        : r.block.task.state}</span
                    >
                    {#if r.block.task.priority}<span class="prio">[#{r.block.task.priority}]</span>{/if}
                  {/if}
                  <span class:closed={r.block.task?.state === 'DONE' || r.block.task?.state === 'CANCELED'}
                    ><Nodes nodes={r.block.content} /></span
                  >
                  <!-- the sub-blocks come with the row only for `children: yes` -->
                  {#each r.block.children as child (child.path.join('.'))}
                    <Block block={child} docName={r.page} viewKey={`${id}-${r.id}`} />
                  {/each}
                {:else if c === 'text' && r.page_blocks.length}
                  <!-- a page row of a `children: yes` query: the page's own blocks -->
                  {#each r.page_blocks as b (b.path.join('.'))}
                    <Block block={b} docName={r.page} viewKey={`${id}-${r.id}`} />
                  {/each}
                {:else}
                  {cell(r, c)}
                {/if}
              </td>
            {/each}
          </tr>
        {/each}
      {/each}
    </tbody>
  </table>
</div>

<style>
  /* a wide table scrolls inside its own box instead of pushing the page sideways */
  .wrap {
    overflow-x: auto;
  }
  .table {
    border-collapse: collapse;
    width: 100%;
    font-size: 0.92em;
  }
  /* a table of tasks: a header the eye finds, roomy rows, columns that line up */
  .table.tasks thead th {
    font-size: 0.75em;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
    border-bottom: 2px solid var(--border);
    white-space: nowrap;
  }
  .table.tasks thead th button {
    color: inherit;
    text-transform: inherit;
    letter-spacing: inherit;
    font-size: inherit;
    font-weight: 650;
    padding: 0.3em 0;
  }
  .table.tasks td {
    padding: 0.55em 0.7em;
    vertical-align: middle;
  }
  .table.tasks tbody tr:hover td {
    background: var(--chip);
  }
  .table.tasks td.text {
    min-width: 18em;
    width: auto;
    white-space: normal;
  }
  .table.tasks th.text {
    width: auto;
  }
  td.late {
    color: var(--danger);
    font-weight: 600;
  }
  td.col-task,
  td.col-priority,
  td.col-deadline,
  td.col-scheduled,
  td.col-prop-created {
    white-space: nowrap;
  }
  .prio-chip {
    display: inline-block;
    min-width: 1.6em;
    text-align: center;
    padding: 0.05em 0.4em;
    border-radius: 6px;
    background: var(--chip);
    font-weight: 700;
    font-size: 0.85em;
  }
  .prio-chip[data-prio='A'] {
    color: var(--danger);
  }
  th,
  td {
    border-bottom: 1px solid var(--border);
    padding: 0.25em 0.6em;
    text-align: left;
    vertical-align: top;
  }
  /* compact columns keep their natural width on one line (long values are cut with an ellipsis, the full
     value is the tooltip); the text column takes the rest and is the only one that wraps */
  thead th:not(.text),
  td:not(.text) {
    white-space: nowrap;
    max-width: 16em;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  th.text,
  td.text {
    width: 100%;
  }
  th button {
    border: none;
    background: none;
    font-weight: 650;
    cursor: pointer;
    color: var(--text);
  }
  tr.group th {
    background: var(--hover, rgba(127, 127, 127, 0.12));
    padding: 0.35em 0.6em;
    font-weight: 650;
    border-top: 2px solid var(--border);
  }
  tr.group .n {
    font-weight: 400;
    opacity: 0.6;
    margin-left: 0.4em;
  }
  tr.group .tog {
    border: none;
    background: none;
    font: inherit;
    color: inherit;
    cursor: pointer;
    padding: 0;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 0.35em;
    font-size: 0.8em;
    font-weight: 700;
    letter-spacing: 0.04em;
    padding: 0.05em 0.6em 0.05em 0.3em;
    border-radius: 999px;
    background: color-mix(in srgb, var(--c) 18%, var(--chip));
    margin-right: 0.3em;
  }
  .state::before {
    content: '';
    width: 1em;
    height: 1em;
    border-radius: 50%;
    background: var(--c);
  }
  .prio {
    color: var(--muted);
    margin-right: 0.3em;
  }
  .closed {
    text-decoration: line-through;
    color: var(--muted);
  }
</style>
