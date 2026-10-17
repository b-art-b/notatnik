<script lang="ts">
  import { backend, type AgendaItem, type AgendaView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { app } from '../stores/app.svelte'
  import { shownDate } from '../stores/format.svelte'
  import SourcedTask from './SourcedTask.svelte'
  import QueryBlock from './QueryBlock.svelte'
  import ValueInput from './ValueInput.svelte'
  import { GROUPS, SORTS, groupItems, sortItems } from '../agenda'

  let ns = $state('')
  let tag = $state('')
  let priority = $state('')
  // `sections` is the classic agenda; the others show the same tasks through the query views
  type Mode = 'sections' | 'list' | 'table' | 'kanban' | 'timeline'
  const MODES: Mode[] = ['sections', 'list', 'table', 'kanban', 'timeline']
  let mode = $state<Mode>('sections')
  let showDone = $state(false)
  let group = $state('')
  let sort = $state('')
  let data = $state<AgendaView | null>(null)
  try {
    const saved = JSON.parse(localStorage.getItem('notatnik-agenda-filters') ?? '{}')
    ns = saved.ns ?? ''
    tag = saved.tag ?? ''
    priority = saved.priority ?? ''
    if (MODES.includes(saved.mode)) mode = saved.mode
    showDone = saved.showDone === true
    group = typeof saved.group === 'string' ? saved.group : ''
    sort = typeof saved.sort === 'string' ? saved.sort : ''
  } catch {
    /* defaults */
  }

  // the agenda's filters as a query: what the other views run (and what "show query" displays)
  const queryText = $derived.by(() => {
    const name = (n: string) => (/^[\w./-]+$/.test(n) ? n : `[[${n}]]`)
    // a board needs the done and canceled columns filled: a card moved there must stay on the board
    const lines = [`task: ${showDone || mode === 'kanban' ? 'any' : 'todo, later, doing, now, waiting'}`]
    if (ns.trim()) lines.push(`ns: ${name(ns.trim().replace(/^#/, ''))}`)
    if (tag.trim()) lines.push(`tag: ${name(tag.trim().replace(/^#/, ''))}`)
    if (priority) lines.push(`priority: ${priority}`)
    lines.push(`sort: ${sort || 'deadline asc'}`)
    if (group) lines.push(`group: ${group}`)
    return lines.join('\n')
  })

  // remember the choices on this device
  $effect(() => {
    try {
      localStorage.setItem(
        'notatnik-agenda-filters',
        JSON.stringify({ ns, tag, priority, mode, showDone, group, sort }),
      )
    } catch {
      /* not remembered */
    }
  })

  $effect(() => {
    void bus.version
    if (mode !== 'sections') return
    const args = { ns: ns.trim() || null, tag: tag.trim() || null, priority: priority || null }
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

{#snippet items(raw: AgendaItem[])}
  {#each groupItems(sort ? sortItems(raw, sort) : raw, group) as g (g.key)}
    {#if g.label}<h4 class="group" data-testid="agenda-group-head">
        {g.label} <span class="n">{g.items.length}</span>
      </h4>{/if}
    {@render rows(g.items)}
  {/each}
{/snippet}

{#snippet rows(list: AgendaItem[])}
  {#each list as it (it.page + it.block.id)}
    <SourcedTask
      block={it.block}
      page={it.page}
      breadcrumbs={it.breadcrumbs}
      viewKey={`agenda-${it.page}-${it.block.id}`}
    />
  {/each}
{/snippet}

<h1>Agenda</h1>
<div class="modes" role="group" aria-label="Agenda view">
  {#each MODES as m (m)}
    <button class="mode" class:on={mode === m} aria-pressed={mode === m} onclick={() => (mode = m)}
      >{m}</button
    >
  {/each}
</div>
<div class="filters" role="search" aria-label="Agenda filters">
  <!-- page suggestions: a namespace and a tag are pages; picking one writes its plain name -->
  <label
    >Namespace <ValueInput bind:value={ns} label="Namespace" placeholder="work" pageFormat="name" /></label
  >
  <label>Tag <ValueInput bind:value={tag} label="Tag" placeholder="urgent" pageFormat="name" /></label>
  <label
    >Priority
    <select bind:value={priority}
      ><option value="">any</option>
      {#each app.graph?.priority_labels ?? [] as lbl (lbl)}
        <option value={lbl}>{lbl}</option>
      {/each}</select
    >
  </label>
  {#if mode !== 'sections' && mode !== 'kanban'}
    <label><input type="checkbox" bind:checked={showDone} data-testid="agenda-done" /> Show done</label>
  {/if}
  <label
    >Group
    <select bind:value={group} data-testid="agenda-group">
      {#each GROUPS as g (g.value)}<option value={g.value}>{g.label}</option>{/each}
    </select>
  </label>
  <label
    >Sort
    <select bind:value={sort} data-testid="agenda-sort">
      {#each SORTS as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
    </select>
  </label>
</div>
{#if mode !== 'sections'}
  <QueryBlock text={queryText} forcedView={mode} />
{:else if data}
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
      <h3 data-day={day.date}>{shownDate(day.date)}</h3>
      {@render items(day.items)}
    {/each}
  </section>
  <section data-section="later">
    <h2>Later <span class="n">{data.later.length}</span></h2>
    {@render items(data.later)}
  </section>
  <section data-section="undated">
    <h2>No date <span class="n">{data.undated.length}</span></h2>
    {@render items(data.undated)}
  </section>
{/if}

<style>
  .modes {
    display: flex;
    gap: 0.3em;
    margin-bottom: 0.8em;
  }
  .mode {
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    border-radius: 6px;
    padding: 0.15em 0.7em;
    cursor: pointer;
  }
  .mode.on {
    background: var(--selection);
    color: var(--text);
    border-color: transparent;
    font-weight: 600;
  }
  .filters {
    flex-wrap: wrap;
    align-items: center;
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
  h4.group {
    font-size: 0.8em;
    font-weight: 600;
    color: var(--muted);
    margin: 0.9em 0 0.2em 34px;
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
</style>
