<script lang="ts">
  import type { ResultRow } from '../../api'
  import { setProperty } from '../../commands/exec'
  import { jumpToBlock } from '../../jump'
  import { app } from '../../stores/app.svelte'
  import { getDoc } from '../../stores/pages.svelte'
  import { addMonths, monthGrid, moveDateValue, weekOf } from '../../views'
  import Nodes from '../Nodes.svelte'

  let { rows }: { rows: ResultRow[] } = $props()
  const weekStart = $derived(app.graph?.week_start === 'sunday' ? 'sunday' : 'monday')
  let mode = $state<'month' | 'week'>('month')
  let field = $state<'deadline' | 'scheduled'>(
    rows.some((r) => r.deadline) || !rows.some((r) => r.scheduled) ? 'deadline' : 'scheduled',
  )
  let month = $state((app.today || new Date().toISOString().slice(0, 10)).slice(0, 7))
  let weekDay = $state(app.today || new Date().toISOString().slice(0, 10))
  const valueOf = (r: ResultRow) => r.block?.task?.[field] ?? null
  const dated = $derived(rows.filter((r) => valueOf(r)))
  const undated = $derived(rows.filter((r) => !valueOf(r)))
  const weeks = $derived(mode === 'month' ? monthGrid(month, weekStart) : [weekOf(weekDay, weekStart)])
  const on = (date: string) => dated.filter((r) => valueOf(r)?.slice(0, 10) === date)

  async function moveTo(r: ResultRow, date: string) {
    const v = valueOf(r)
    if (!r.block || !v || v.slice(0, 10) === date) return
    await setProperty(getDoc(r.page), r.block.path, field, moveDateValue(v, date))
  }
  /** Alt+← / Alt+→: one day earlier or later. */
  function key(e: KeyboardEvent, r: ResultRow) {
    if (e.key === 'Enter' && e.target === e.currentTarget) return open(r)
    if (!e.altKey || (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight')) return
    const v = valueOf(r)
    if (!v) return
    e.preventDefault()
    const d = new Date(`${v.slice(0, 10)}T00:00:00Z`)
    d.setUTCDate(d.getUTCDate() + (e.key === 'ArrowLeft' ? -1 : 1))
    void moveTo(r, d.toISOString().slice(0, 10))
  }
  /** A click opens the note the task lives in, with the task flashed. */
  function open(r: ResultRow) {
    if (r.block) jumpToBlock(r.page, r.block.path)
  }

  // Pointer-event dragging (see KanbanView): works in every webview, unlike HTML5 drag and drop.
  const THRESHOLD = 5
  let press: { id: string; x: number; y: number } | null = null
  let drag = $state<{ id: string; over: string | null } | null>(null)
  let justDragged = false
  const dayAt = (x: number, y: number): string | null =>
    document.elementFromPoint(x, y)?.closest<HTMLElement>('td[data-date]')?.dataset.date ?? null
  function down(e: PointerEvent, r: ResultRow) {
    if (e.button !== 0) return
    press = { id: r.id, x: e.clientX, y: e.clientY }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }
  function move(e: PointerEvent) {
    if (!press) return
    if (!drag && Math.hypot(e.clientX - press.x, e.clientY - press.y) < THRESHOLD) return
    drag = { id: press.id, over: dayAt(e.clientX, e.clientY) }
  }
  function up(e: PointerEvent, r: ResultRow) {
    const d = drag
    press = null
    drag = null
    if (!d) return
    justDragged = true
    setTimeout(() => (justDragged = false), 0)
    const to = dayAt(e.clientX, e.clientY)
    if (to && d.id === r.id) void moveTo(r, to)
  }
  function step(n: number) {
    if (mode === 'month') month = addMonths(month, n)
    else {
      const d = new Date(`${weekDay}T00:00:00Z`)
      d.setUTCDate(d.getUTCDate() + 7 * n)
      weekDay = d.toISOString().slice(0, 10)
    }
  }
</script>

<div class="cal" data-testid="query-timeline">
  <div class="bar">
    <button aria-label="Previous" onclick={() => step(-1)}>‹</button>
    <strong>{mode === 'month' ? month : `week of ${weeks[0][0].date}`}</strong>
    <button aria-label="Next" onclick={() => step(1)}>›</button>
    <select aria-label="Range" bind:value={mode}
      ><option value="month">Month</option><option value="week">Week</option></select
    >
    <select aria-label="Date field" bind:value={field}
      ><option value="deadline">deadline</option><option value="scheduled">scheduled</option></select
    >
  </div>
  <table>
    <tbody>
      {#each weeks as w, i (i)}
        <tr>
          {#each w as d (d.date)}
            <td
              class:out={!d.inMonth}
              class:today={d.date === app.today}
              class:over={drag?.over === d.date}
              data-date={d.date}
            >
              <div class="num">{Number(d.date.slice(8))}</div>
              {#each on(d.date) as r (r.id)}
                <div
                  class="ev"
                  role="button"
                  tabindex="0"
                  class:dragging={drag?.id === r.id}
                  data-ev={r.id}
                  title="Click to open the note. Drag to another day, or Alt+← / Alt+→"
                  onpointerdown={(e) => down(e, r)}
                  onpointermove={move}
                  onpointerup={(e) => up(e, r)}
                  onpointercancel={() => ((press = null), (drag = null))}
                  onclick={() => !justDragged && open(r)}
                  onkeydown={(e) => key(e, r)}
                >
                  {#if r.block}<Nodes nodes={r.block.content} />{:else}{r.page}{/if}
                </div>
              {/each}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
  {#if undated.length}<p class="muted">{undated.length} without a {field} date</p>{/if}
</div>

<style>
  .bar {
    display: flex;
    gap: 0.5em;
    align-items: center;
    margin-bottom: 0.3em;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  td {
    border: 1px solid var(--border);
    vertical-align: top;
    height: 5.5em;
    padding: 2px 4px;
    font-size: 0.82em;
    overflow: hidden;
  }
  td.over {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .ev {
    touch-action: none;
    user-select: none;
    cursor: pointer;
  }
  .ev.dragging {
    opacity: 0.6;
  }
  td.out {
    background: var(--code-bg);
    color: var(--muted);
  }
  td.today {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .num {
    font-size: 0.75em;
    color: var(--muted);
  }
  .ev {
    background: var(--selection);
    border-radius: 4px;
    padding: 0 3px;
    margin: 1px 0;
    cursor: grab;
  }
</style>
