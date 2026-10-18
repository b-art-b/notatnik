<script lang="ts">
  import PageIcon from './PageIcon.svelte'
  import { backend, type JournalDay } from '../api'
  import { app } from '../stores/app.svelte'
  import PageContent from './PageContent.svelte'
  import DayEvents from './DayEvents.svelte'
  import { pageHash } from '../route'
  import { shownDate } from '../stores/format.svelte'

  let days = $state<JournalDay[]>([])
  let loading = false
  let sentinel: HTMLDivElement | undefined = $state()

  async function more() {
    if (loading) return
    loading = true
    try {
      const before = days.length ? days[days.length - 1].date : null
      const next = await backend.call('journals', { before, limit: 14 })
      days = [...days, ...next]
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void app.today
    days = []
    void more()
  })
  $effect(() => {
    if (!sentinel) return
    const io = new IntersectionObserver((es) => es.some((e) => e.isIntersecting) && void more())
    io.observe(sentinel)
    return () => io.disconnect()
  })

  // runs of empty days collapse into one line (today is always shown)
  const rows = $derived.by(() => {
    const out: ({ kind: 'day'; day: JournalDay } | { kind: 'gap'; from: string; to: string; n: number })[] =
      []
    for (const d of days) {
      if (d.exists || d.date === app.today) out.push({ kind: 'day', day: d })
      else {
        const last = out[out.length - 1]
        if (last?.kind === 'gap') {
          last.to = d.date
          last.n++
        } else out.push({ kind: 'gap', from: d.date, to: d.date, n: 1 })
      }
    }
    return out
  })
</script>

<div class="journals" data-testid="journals">
  {#each rows as r, i (r.kind === 'day' ? r.day.date : `gap-${r.from}-${i}`)}
    {#if r.kind === 'day'}
      <section class="day">
        <PageContent
          name={r.day.name}
          viewKey={`journal-${r.day.name}`}
          showBacklinks={false}
          compact={r.day.date !== app.today}
          titleLink
          hideProps
        />
        {#if r.day.date !== app.today}<DayEvents date={r.day.date} />{/if}
        {#if r.day.date !== app.today}<a class="day-title" href={pageHash(r.day.name)}
            ><PageIcon name={r.day.name} journal />{shownDate(r.day.name)}</a
          >{/if}
      </section>
    {:else}
      <div class="gap muted">
        {r.n === 1
          ? `${shownDate(r.from)}: nothing written`
          : `${shownDate(r.to)} – ${shownDate(r.from)}: nothing written (${r.n} days)`}
      </div>
    {/if}
  {/each}
  <div bind:this={sentinel} style="height: 1px"></div>
  <button class="more" onclick={more}>Load older days</button>
</div>

<style>
  /* one day after another, set apart by a hairline and plenty of air, like a page of a daybook */
  .day {
    display: flex;
    flex-direction: column;
    padding-top: 2.2em;
    margin-top: 2.4em;
    border-top: 1px solid var(--border);
  }
  .day:first-child {
    padding-top: 0.4em;
    margin-top: 0;
    border-top: none;
  }
  .day-title {
    order: -1;
    color: var(--text);
    font-size: 2em;
    font-weight: 700;
    letter-spacing: -0.022em;
    line-height: 1.2;
    text-decoration: none;
    margin-bottom: 0.7em;
  }
  .day-title:hover {
    text-decoration: underline;
    text-decoration-thickness: 2px;
    text-underline-offset: 0.12em;
  }
  .gap {
    font-size: 0.85em;
    margin: 1.4em 0 0;
  }
  .more {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    margin-top: 2em;
  }
</style>
