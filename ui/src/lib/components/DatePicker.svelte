<script lang="ts">
  // Small popup for setting a date (slash commands, task chips). It takes focus, so the editor loses
  // it; the parent refocuses the editor when `onpick`/`oncancel` runs. Dates are always shown and typed
  // as YYYY-MM-DD: there is no native `<input type="date">`, which would show the OS locale format.
  import { onMount } from 'svelte'
  import { app } from '../stores/app.svelte'
  import { parseNaturalDate } from '../dates'
  import { addMonths, monthGrid } from '../views'

  interface Props {
    title: string
    /** Prefilled date (YYYY-MM-DD) when editing an existing one. */
    initial?: string
    onpick: (iso: string) => void
    oncancel: () => void
    /** When given, a "Clear" button removes the date. */
    onclear?: () => void
  }
  let { title, initial = '', onpick, oncancel, onclear }: Props = $props()

  const today = $derived(app.today || new Date().toISOString().slice(0, 10))
  // `initial` is only the starting value; reading it once is intended
  // svelte-ignore state_referenced_locally
  let typed = $state(initial)
  // `$derived`: recomputed from `typed`, never written to
  const parsed = $derived(typed.trim() ? parseNaturalDate(typed, today) : null)
  let textInput: HTMLInputElement

  // The calendar follows the typed date; the arrows move it by month. `shown` is the month being looked at
  // (`YYYY-MM`); `moved` records that the user navigated, so typing no longer jumps the view back.
  let moved = $state<string | null>(null)
  const shown = $derived(moved ?? (parsed ?? today).slice(0, 7))
  const weekStart = $derived(app.graph?.week_start === 'sunday' ? 'sunday' : 'monday')
  const weeks = $derived(monthGrid(shown, weekStart))
  const heads = $derived(
    weekStart === 'sunday'
      ? ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa']
      : ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'],
  )

  onMount(() => {
    textInput.focus()
    textInput.select()
  })

  function key(e: KeyboardEvent) {
    e.stopPropagation() // keep Enter/Escape away from the editor and the global handler
    if (e.key === 'Escape') {
      e.preventDefault()
      oncancel()
    } else if (e.key === 'Enter' && parsed) {
      e.preventDefault()
      onpick(parsed)
    }
  }

  const quick = $derived(
    (['today', 'tomorrow', 'next monday', 'in 1 week'] as const).map((p) => ({
      label: p === 'in 1 week' ? 'next week' : p,
      iso: parseNaturalDate(p, today) ?? today,
    })),
  )
</script>

<!-- a click inside must not reach the block's own click handler (it would re-open the editor) -->
<div
  class="date-picker"
  role="dialog"
  aria-label={title}
  tabindex="-1"
  data-testid="date-picker"
  onkeydown={key}
  onclick={(e) => e.stopPropagation()}
>
  <strong>{title}</strong>
  <input
    bind:this={textInput}
    bind:value={typed}
    oninput={() => (moved = null)}
    aria-label="Date (YYYY-MM-DD, or e.g. tomorrow, next friday, +3d)"
    placeholder="YYYY-MM-DD, tomorrow, +3d"
  />
  <div class="preview" aria-live="polite">
    {#if parsed}{parsed}{:else if typed.trim()}not a date{:else}&nbsp;{/if}
  </div>
  <div class="cal-bar">
    <button type="button" aria-label="Previous month" onclick={() => (moved = addMonths(shown, -1))}>‹</button
    >
    <strong data-testid="picker-month">{shown}</strong>
    <button type="button" aria-label="Next month" onclick={() => (moved = addMonths(shown, 1))}>›</button>
  </div>
  <table class="cal" aria-label="Calendar">
    <thead>
      <tr
        >{#each heads as h (h)}<th scope="col">{h}</th>{/each}</tr
      >
    </thead>
    <tbody>
      {#each weeks as w, i (i)}
        <tr>
          {#each w as d (d.date)}
            <td>
              <button
                type="button"
                class="day"
                class:out={!d.inMonth}
                class:today={d.date === today}
                class:picked={d.date === parsed}
                title={d.date}
                aria-label={d.date}
                onclick={() => onpick(d.date)}>{Number(d.date.slice(8))}</button
              >
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
  <div class="quick">
    {#each quick as q (q.label)}
      <button type="button" title={q.iso} onclick={() => onpick(q.iso)}>{q.label}</button>
    {/each}
    {#if onclear}<button type="button" onclick={onclear}>Clear</button>{/if}
  </div>
</div>

<style>
  .date-picker {
    display: flex;
    flex-direction: column;
    gap: 0.35em;
    padding: 0.6em;
    width: 18em;
    max-width: 100%;
    box-sizing: border-box;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.2);
    font-size: 0.9em;
  }
  .preview {
    color: var(--muted);
    min-height: 1.2em;
  }
  .cal-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .cal {
    border-collapse: collapse;
    width: 100%;
    text-align: center;
  }
  .cal th {
    color: var(--muted);
    font-weight: 500;
    font-size: 0.8em;
  }
  .day {
    width: 100%;
    border: none;
    background: none;
    color: inherit;
    cursor: pointer;
    padding: 0.2em 0;
    border-radius: 4px;
  }
  .day:hover {
    background: var(--chip);
  }
  .day.out {
    color: var(--muted);
    opacity: 0.6;
  }
  .day.today {
    outline: 1px solid var(--accent);
  }
  .day.picked {
    background: var(--accent);
    color: white;
  }
  .quick {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3em;
  }
</style>
