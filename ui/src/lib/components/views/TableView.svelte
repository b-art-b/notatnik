<script lang="ts">
  import type { ResultRow } from '../../api'
  import { pageHash } from '../../route'
  import { setProperty } from '../../commands/exec'
  import { getDoc } from '../../stores/pages.svelte'
  import Nodes from '../Nodes.svelte'
  import Block from '../Block.svelte'
  import { shownDate } from '../../stores/format.svelte'
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

<table class="table" data-testid="query-table">
  <thead>
    <tr>
      {#each columns as c (c)}
        <th
          scope="col"
          class:text={c === 'text'}
          aria-sort={sortCol === c ? (desc ? 'descending' : 'ascending') : 'none'}
        >
          <button onclick={() => header(c)}
            >{c.replace(/^prop\./, '')}{sortCol === c ? (desc ? ' ▼' : ' ▲') : ''}</button
          >
        </th>
      {/each}
    </tr>
  </thead>
  <tbody>
    {#each groups as g (g.label)}
      {#if g.label !== null}
        <tr class="group" data-group={g.label}>
          <th scope="rowgroup" colspan={columns.length}>
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
          {#each columns as c (c)}
            <td
              class:text={c === 'text'}
              title={c === 'text' ? undefined : cell(r, c)}
              ondblclick={() => c.startsWith('prop.') && (editing = { row: r.id, col: c })}
            >
              {#if editing && editing.row === r.id && editing.col === c}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  autofocus
                  aria-label={`Edit ${c}`}
                  value={cell(r, c)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter') void save(r, c, (e.currentTarget as HTMLInputElement).value)
                    if (e.key === 'Escape') editing = null
                  }}
                  onblur={(e) => void save(r, c, e.currentTarget.value)}
                />
              {:else if c === 'page'}
                <a href={pageHash(r.page)}>{pageLabel(r.page)}</a>
              {:else if c === 'text' && r.block}
                <!-- the list view shows a result as a full Block (state badge and all); a cell shows the text, so a task
                   gets a read-only badge here (change the state in the list view or on its page) -->
                {#if r.block.task}
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

<style>
  .table {
    border-collapse: collapse;
    width: 100%;
    font-size: 0.92em;
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
