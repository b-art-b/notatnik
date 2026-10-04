<script lang="ts">
  import { backend, type JournalDay } from '../api'
  import { app } from '../stores/app.svelte'
  import PageContent from './PageContent.svelte'
  import { pageHash } from '../route'
  import { formatDate } from '../dates'

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
        />
        {#if r.day.date !== app.today}<a class="day-title" href={pageHash(r.day.name)}
            >{formatDate(r.day.name)}</a
          >{/if}
      </section>
    {:else}
      <div class="gap muted">
        {r.n === 1
          ? `${formatDate(r.from)}: nothing written`
          : `${formatDate(r.to)} – ${formatDate(r.from)}: nothing written (${r.n} days)`}
      </div>
    {/if}
  {/each}
  <div bind:this={sentinel} style="height: 1px"></div>
  <button class="more" onclick={more}>Load older days</button>
</div>

<style>
  .day {
    margin-bottom: 1.5em;
    display: flex;
    flex-direction: column;
  }
  .day-title {
    order: -1;
    font-weight: 650;
    font-size: 1.2em;
    margin-bottom: 0.2em;
  }
  .gap {
    font-size: 0.85em;
    margin: 0.4em 0;
  }
  .more {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
</style>
