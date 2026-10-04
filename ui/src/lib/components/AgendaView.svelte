<script lang="ts">
  import { backend, type AgendaItem, type AgendaView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { formatDate } from '../dates'
  import Block from './Block.svelte'

  let ns = $state('')
  let tag = $state('')
  let priority = $state('')
  let data = $state<AgendaView | null>(null)
  try {
    const saved = JSON.parse(localStorage.getItem('notatnik-agenda-filters') ?? '{}')
    ns = saved.ns ?? ''
    tag = saved.tag ?? ''
    priority = saved.priority ?? ''
  } catch {
    /* defaults */
  }

  $effect(() => {
    void bus.version
    const args = { ns: ns.trim() || null, tag: tag.trim() || null, priority: priority || null }
    try {
      localStorage.setItem('notatnik-agenda-filters', JSON.stringify({ ns, tag, priority }))
    } catch {
      /* not remembered */
    }
    let cancelled = false
    const timer = setTimeout(async () => {
      try {
        const r = await backend.call('agenda', args)
        if (!cancelled) data = r
      } catch {
        /* an invalid filter keeps the previous result */
      }
    }, 100)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  })
</script>

{#snippet items(list: AgendaItem[])}
  {#each list as it (it.page + it.block.id)}
    <div class="item" data-testid="agenda-item">
      <div class="crumbs">
        {it.page}{it.breadcrumbs.length ? ' › ' + it.breadcrumbs.join(' › ') : ''}{it.date
          ? ` · ${it.date}`
          : ''}
      </div>
      <Block block={it.block} docName={it.page} viewKey={`agenda-${it.page}-${it.block.id}`} />
    </div>
  {/each}
{/snippet}

<h1>Agenda</h1>
<div class="filters" role="search" aria-label="Agenda filters">
  <label>Namespace <input bind:value={ns} placeholder="work" /></label>
  <label>Tag <input bind:value={tag} placeholder="urgent" /></label>
  <label
    >Priority
    <select bind:value={priority}
      ><option value="">any</option><option value="high">high</option><option value="medium">medium</option
      ><option value="low">low</option></select
    >
  </label>
</div>
{#if data}
  <section data-section="overdue">
    <h2 class:warn={data.overdue.length > 0}>Overdue <span class="n">{data.overdue.length}</span></h2>
    {@render items(data.overdue)}
  </section>
  <section data-section="today">
    <h2>Today <span class="n">{data.today_items.length}</span></h2>
    {@render items(data.today_items)}
  </section>
  <section data-section="upcoming">
    <h2>Next 7 days <span class="n">{data.upcoming.reduce((n, d) => n + d.items.length, 0)}</span></h2>
    {#each data.upcoming as day (day.date)}
      <h3 data-day={day.date}>{formatDate(day.date)}</h3>
      {@render items(day.items)}
    {/each}
  </section>
  <section data-section="undated">
    <h2>No date <span class="n">{data.undated.length}</span></h2>
    {@render items(data.undated)}
  </section>
{/if}

<style>
  .filters {
    display: flex;
    gap: 1em;
    margin-bottom: 1em;
    font-size: 0.9em;
  }
  h2 {
    font-size: 1.05em;
    border-bottom: 1px solid var(--border);
    margin-top: 1.4em;
  }
  h2.warn {
    color: var(--danger);
  }
  h3 {
    font-size: 0.9em;
    color: var(--muted);
    margin: 0.8em 0 0.2em;
  }
  .n {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.8em;
    font-weight: 400;
  }
  .crumbs {
    font-size: 0.72em;
    color: var(--muted);
    margin-left: 34px;
  }
</style>
