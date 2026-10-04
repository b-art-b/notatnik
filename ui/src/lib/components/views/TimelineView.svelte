<script lang="ts">
  import type { ResultRow } from '../../api'
  import { setProperty } from '../../commands/exec'
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
  let dragging = $state<string | null>(null)

  const valueOf = (r: ResultRow) => r.block?.task?.[field] ?? null
  const dated = $derived(rows.filter((r) => valueOf(r)))
  const undated = $derived(rows.filter((r) => !valueOf(r)))
  const weeks = $derived(mode === 'month' ? monthGrid(month, weekStart) : [weekOf(weekDay, weekStart)])
  const on = (date: string) => dated.filter((r) => valueOf(r)?.slice(0, 10) === date)

  async function drop(date: string) {
    const r = rows.find((x) => x.id === dragging)
    dragging = null
    const v = r ? valueOf(r) : null
    if (!r || !r.block || !v || v.slice(0, 10) === date) return
    await setProperty(getDoc(r.page), r.block.path, field, moveDateValue(v, date))
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
              data-date={d.date}
              ondragover={(e) => e.preventDefault()}
              ondrop={(e) => (e.preventDefault(), void drop(d.date))}
            >
              <div class="num">{Number(d.date.slice(8))}</div>
              {#each on(d.date) as r (r.id)}
                <div
                  class="ev"
                  role="button"
                  tabindex="0"
                  draggable="true"
                  data-ev={r.id}
                  ondragstart={() => (dragging = r.id)}
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
