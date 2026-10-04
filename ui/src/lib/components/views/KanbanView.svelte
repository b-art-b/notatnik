<script lang="ts">
  import type { ResultRow, TaskState } from '../../api'
  import { setProperty, taskStateOp } from '../../commands/exec'
  import { getDoc } from '../../stores/pages.svelte'
  import { kanbanColumns, kanbanKey } from '../../views'
  import Nodes from '../Nodes.svelte'

  let { rows, groupBy }: { rows: ResultRow[]; groupBy: string | null } = $props()
  const columns = $derived(kanbanColumns(rows, groupBy))
  const byTask = $derived(!groupBy || groupBy === 'task')
  let dragging = $state<string | null>(null)

  async function move(r: ResultRow, to: string) {
    if (!r.block || kanbanKey(r, groupBy) === to) return
    const doc = getDoc(r.page)
    if (byTask) {
      await doc.apply([taskStateOp(r.block.path, to.toUpperCase() as TaskState)])
    } else if (groupBy?.startsWith('prop.')) {
      await setProperty(doc, r.block.path, groupBy.slice(5), to === '(none)' ? null : to)
    }
  }

  function cardKey(e: KeyboardEvent, r: ResultRow) {
    if (!e.altKey || (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight')) return
    e.preventDefault()
    const i = columns.indexOf(kanbanKey(r, groupBy))
    const j = e.key === 'ArrowLeft' ? i - 1 : i + 1
    if (columns[j]) void move(r, columns[j])
  }
</script>

<div class="board" data-testid="query-kanban">
  {#each columns as col (col)}
    {@const cards = rows.filter((r) => kanbanKey(r, groupBy) === col)}
    <section
      class="col"
      data-col={col}
      aria-label={`${col} (${cards.length})`}
      ondragover={(e) => e.preventDefault()}
      ondrop={(e) => {
        e.preventDefault()
        const r = rows.find((x) => x.id === dragging)
        dragging = null
        if (r) void move(r, col)
      }}
    >
      <h4>{col} <span class="n">{cards.length}</span></h4>
      {#each cards as r (r.id)}
        <div
          class="card"
          role="button"
          tabindex="0"
          draggable="true"
          data-card={r.id}
          ondragstart={() => (dragging = r.id)}
          onkeydown={(e) => cardKey(e, r)}
          title="Alt+← / Alt+→ moves the card"
        >
          {#if r.block}<Nodes nodes={r.block.content} />{:else}{r.page}{/if}
          <div class="meta">{r.page}{r.deadline ? ` · 📅 ${r.deadline}` : ''}</div>
        </div>
      {/each}
    </section>
  {/each}
</div>

<style>
  .board {
    display: flex;
    gap: 0.6em;
    overflow-x: auto;
    align-items: flex-start;
  }
  .col {
    min-width: 180px;
    flex: 1;
    background: var(--code-bg);
    border-radius: 8px;
    padding: 0.4em;
    min-height: 60px;
  }
  h4 {
    margin: 0 0 0.4em;
    font-size: 0.8em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .n {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.4em;
  }
  .card {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35em 0.5em;
    margin-bottom: 0.35em;
    cursor: grab;
    font-size: 0.9em;
  }
  .meta {
    font-size: 0.72em;
    color: var(--muted);
  }
</style>
