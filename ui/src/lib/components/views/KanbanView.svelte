<script lang="ts">
  import type { ResultRow, TaskState } from '../../api'
  import { setProperty, taskStateOp } from '../../commands/exec'
  import { jumpToBlock } from '../../jump'
  import { getDoc } from '../../stores/pages.svelte'
  import { kanbanColumns, kanbanKey } from '../../views'
  import Nodes from '../Nodes.svelte'

  let { rows, groupBy }: { rows: ResultRow[]; groupBy: string | null } = $props()
  const columns = $derived(kanbanColumns(rows, groupBy))
  const byTask = $derived(!groupBy || groupBy === 'task')
  // Cards move by state or by a property; a board grouped by page or date has nothing to change by moving
  const movable = $derived(byTask || !!groupBy?.startsWith('prop.'))

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
    if (e.key === 'Enter' && e.target === e.currentTarget) return open(r)
    if (!e.altKey || (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight')) return
    e.preventDefault()
    step(r, e.key === 'ArrowLeft' ? -1 : 1)
  }

  function step(r: ResultRow, by: -1 | 1) {
    const j = columns.indexOf(kanbanKey(r, groupBy)) + by
    if (columns[j]) void move(r, columns[j])
  }

  /** A click on a card opens the note the task lives in, with the task flashed (like the agenda). */
  function open(r: ResultRow) {
    if (r.block) jumpToBlock(r.page, r.block.path)
  }

  // Dragging uses pointer events, not HTML5 drag and drop: they work in every webview (and with touch), and the
  // packaged app's window swallows the HTML5 kind. A press that moves a few pixels starts a drag; the column
  // under the pointer when it is released is where the card goes.
  const THRESHOLD = 5
  let press: { id: string; x: number; y: number } | null = null
  let drag = $state<{ id: string; over: string | null } | null>(null)
  let justDragged = false

  function columnAt(x: number, y: number): string | null {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-col]')
    return el?.dataset.col ?? null
  }
  function down(e: PointerEvent, r: ResultRow) {
    if (e.button !== 0 || !movable || (e.target as HTMLElement).closest('button, a')) return
    press = { id: r.id, x: e.clientX, y: e.clientY }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }
  function moveTo(e: PointerEvent) {
    if (!press) return
    if (!drag && Math.hypot(e.clientX - press.x, e.clientY - press.y) < THRESHOLD) return
    drag = { id: press.id, over: columnAt(e.clientX, e.clientY) }
  }
  function up(e: PointerEvent, r: ResultRow) {
    const d = drag
    press = null
    drag = null
    if (!d) return
    // the click that follows a drag must not open the note
    justDragged = true
    setTimeout(() => (justDragged = false), 0)
    const to = columnAt(e.clientX, e.clientY)
    if (to && d.id === r.id) void move(r, to)
  }
  function click(e: MouseEvent, r: ResultRow) {
    if (justDragged || (e.target as HTMLElement).closest('button')) return
    open(r)
  }
</script>

<div class="board" data-testid="query-kanban">
  {#each columns as col (col)}
    {@const cards = rows.filter((r) => kanbanKey(r, groupBy) === col)}
    <section
      class="col"
      class:over={drag?.over === col}
      data-col={col}
      aria-label={`${col} (${cards.length})`}
    >
      <h4>{col} <span class="n">{cards.length}</span></h4>
      {#each cards as r (r.id)}
        {@const i = columns.indexOf(col)}
        <div
          class="card"
          class:dragging={drag?.id === r.id}
          role="button"
          tabindex="0"
          data-card={r.id}
          data-movable={movable}
          onpointerdown={(e) => down(e, r)}
          onpointermove={moveTo}
          onpointerup={(e) => up(e, r)}
          onpointercancel={() => ((press = null), (drag = null))}
          onclick={(e) => click(e, r)}
          onkeydown={(e) => cardKey(e, r)}
          title={movable
            ? 'Click to open the note. Drag to another column, or use the arrows (Alt+← / Alt+→)'
            : 'Click to open the note (cards can only be moved when the board is grouped by state or by a property)'}
        >
          {#if r.block}<Nodes nodes={r.block.content} />{:else}{r.page}{/if}
          <div class="meta">{r.page}{r.deadline ? ` · 📅 ${r.deadline}` : ''}</div>
          {#if movable}
            <div class="steps">
              <button aria-label={`Move to ${columns[i - 1]}`} disabled={i === 0} onclick={() => step(r, -1)}
                >‹</button
              >
              <button
                aria-label={`Move to ${columns[i + 1]}`}
                disabled={i === columns.length - 1}
                onclick={() => step(r, 1)}>›</button
              >
            </div>
          {/if}
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
  .col.over {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .card.dragging {
    opacity: 0.6;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.18);
  }
  .steps {
    /* faded, not display:none: the arrows stay reachable by keyboard and screen readers */
    opacity: 0;
    display: flex;
    justify-content: flex-end;
    gap: 0.2em;
    margin-top: 0.2em;
  }
  .card:hover .steps,
  .card:focus-within .steps {
    opacity: 1;
  }
  .steps button {
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--text);
    border-radius: 4px;
    width: 1.8em;
    cursor: pointer;
    line-height: 1.3;
  }
  .steps button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .card {
    position: relative;
    touch-action: none;
    user-select: none;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35em 0.5em;
    margin-bottom: 0.35em;
    cursor: grab;
    font-size: 0.9em;
  }
  .card[data-movable='false'] {
    cursor: pointer;
  }
  .meta {
    font-size: 0.72em;
    color: var(--muted);
  }
</style>
