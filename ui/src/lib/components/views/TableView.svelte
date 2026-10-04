<script lang="ts">
  import type { ResultRow } from '../../api'
  import { pageHash } from '../../route'
  import { setProperty } from '../../commands/exec'
  import { getDoc } from '../../stores/pages.svelte'
  import Nodes from '../Nodes.svelte'
  import { cellValue, sortRows } from '../../views'

  let { rows, columns }: { rows: ResultRow[]; columns: string[] } = $props()
  let sortCol = $state<string | null>(null)
  let desc = $state(false)
  let editing = $state<{ row: string; col: string } | null>(null)

  const cell = cellValue
  const sorted = $derived(sortRows(rows, sortCol, desc))

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
        <th scope="col" aria-sort={sortCol === c ? (desc ? 'descending' : 'ascending') : 'none'}>
          <button onclick={() => header(c)}
            >{c.replace(/^prop\./, '')}{sortCol === c ? (desc ? ' ▼' : ' ▲') : ''}</button
          >
        </th>
      {/each}
    </tr>
  </thead>
  <tbody>
    {#each sorted as r (r.id)}
      <tr data-row={r.id}>
        {#each columns as c (c)}
          <td ondblclick={() => c.startsWith('prop.') && (editing = { row: r.id, col: c })}>
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
              <a href={pageHash(r.page)}>{r.page}</a>
            {:else if c === 'text' && r.block}
              <Nodes nodes={r.block.content} />
            {:else}
              {cell(r, c)}
            {/if}
          </td>
        {/each}
      </tr>
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
  th button {
    border: none;
    background: none;
    font-weight: 650;
    cursor: pointer;
    color: var(--text);
  }
</style>
