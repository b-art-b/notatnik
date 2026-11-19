<script lang="ts">
  import { backend, type AgendaItem, type AgendaView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { app } from '../stores/app.svelte'
  import { shownDate } from '../stores/format.svelte'
  import SourcedTask from './SourcedTask.svelte'
  import QueryBlock from './QueryBlock.svelte'
  import CalendarView from './CalendarView.svelte'
  import ValueInput from './ValueInput.svelte'
  import { pageHash } from '../route'
  import { GROUPS, SORTS, groupItems, sortItems } from '../agenda'

  const ISO = /^\d{4}-\d{2}-\d{2}$/
  const HINTS: Record<string, string> = {
    page: 'Tasks are listed under the note they are in (a journal day is a note too).',
    task: 'Tasks are listed by their state.',
    deadline: 'Tasks are listed by their deadline.',
    scheduled: 'Tasks are listed by their scheduled date.',
  }
  let ns = $state('')
  let tag = $state('')
  let priority = $state('')
  // `sections` is the classic agenda; the others show the same tasks through the query views
  type Mode = 'sections' | 'list' | 'table' | 'kanban' | 'timeline' | 'calendar'
  const MODES: Mode[] = ['sections', 'list', 'table', 'kanban', 'timeline', 'calendar']
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
    // the table names the note each task is in, and the day it was made (Settings → Tasks → Creation date), as columns
    if (mode === 'table') lines.push('columns: task, priority, text, deadline, scheduled, page, prop.created')
    lines.push(`sort: ${sort || 'deadline asc'}`)
    if (group) lines.push(`group: ${group}`)
    return lines.join('\n')
  })

  const filtersActive = $derived(ns.trim() !== '' || tag.trim() !== '' || priority !== '')
  function clearFilters() {
    ns = ''
    tag = ''
    priority = ''
  }

  // the numbers in the strip under the title: one chip per section, a click scrolls to it
  const summary = $derived(
    data
      ? [
          { id: 'overdue', label: 'Overdue', n: data.overdue.length, warn: true },
          { id: 'today', label: 'Today', n: data.today_items.length, warn: false },
          {
            id: 'upcoming',
            label: 'Next 7 days',
            n: data.upcoming.reduce((n, d) => n + d.items.length, 0),
            warn: false,
          },
          { id: 'later', label: 'Later', n: data.later.length, warn: false },
          { id: 'undated', label: 'No date', n: data.undated.length, warn: false },
        ]
      : [],
  )
  const allEmpty = $derived(summary.length > 0 && summary.every((x) => x.n === 0))
  function jump(id: string) {
    document.querySelector(`[data-section="${id}"]`)?.scrollIntoView({ block: 'start', behavior: 'smooth' })
  }

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
  {@const groups = groupItems(sort ? sortItems(raw, sort) : raw, group)}
  {#each groups as g (g.key)}
    {#if g.label && !(groups.length === 1 && g.label.startsWith('no '))}
      <h4 class="group" data-testid="agenda-group-head">
        <!-- a page is a link; a date is shown in the device's format, like everywhere else -->
        {#if group === 'page'}
          <a href={pageHash(g.key)}>{ISO.test(g.label) ? shownDate(g.label) : g.label}</a>
        {:else}
          {ISO.test(g.label) ? shownDate(g.label) : g.label}
        {/if}
        <span class="n">{g.items.length}</span>
      </h4>
    {/if}
    {@render rows(g.items)}
  {/each}
{/snippet}

{#snippet rows(list: AgendaItem[])}
  {#each list as it (it.page + it.block.id)}
    <!-- grouped by page the heading already names the note, so the row's own chip would repeat it -->
    <SourcedTask
      block={it.block}
      page={it.page}
      breadcrumbs={it.breadcrumbs}
      viewKey={`agenda-${it.page}-${it.block.id}`}
      chip={group === 'page' ? null : undefined}
    />
  {/each}
{/snippet}

<header class="head">
  <h1>Agenda</h1>
  <!-- a segmented control: one of the views is always on -->
  <div class="modes" role="group" aria-label="Agenda view">
    {#each MODES as m (m)}
      <button class="mode" class:on={mode === m} aria-pressed={mode === m} onclick={() => (mode = m)}
        >{m}</button
      >
    {/each}
  </div>
</header>
<div class="filters" role="search" aria-label="Agenda filters">
  <!-- page suggestions: a namespace and a tag are pages; picking one writes its plain name -->
  <label class="field"
    ><span class="cap">Namespace</span>
    <ValueInput bind:value={ns} label="Namespace" placeholder="work" pageFormat="name" /></label
  >
  <label class="field"
    ><span class="cap">Tag</span>
    <ValueInput bind:value={tag} label="Tag" placeholder="urgent" pageFormat="name" /></label
  >
  <label class="field narrow"
    ><span class="cap">Priority</span>
    <select bind:value={priority}
      ><option value="">any</option>
      {#each app.graph?.priority_labels ?? [] as lbl (lbl)}
        <option value={lbl}>{lbl}</option>
      {/each}</select
    >
  </label>
  {#if mode !== 'calendar'}
    <label class="field narrow"
      ><span class="cap">Group by</span>
      <select bind:value={group} data-testid="agenda-group">
        {#each GROUPS as g (g.value)}<option value={g.value}>{g.label}</option>{/each}
      </select>
    </label>
    <label class="field narrow"
      ><span class="cap">Sort by</span>
      <select bind:value={sort} data-testid="agenda-sort">
        {#each SORTS as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
      </select>
    </label>
  {/if}
  {#if mode !== 'sections' && mode !== 'kanban'}
    <label class="done"
      ><input type="checkbox" bind:checked={showDone} data-testid="agenda-done" /> Show done</label
    >
  {/if}
  {#if filtersActive}
    <button class="clear" type="button" onclick={clearFilters}>Clear filters</button>
  {/if}
</div>
{#if group && mode !== 'calendar'}
  <p class="hint">
    Grouped by {GROUPS.find((g) => g.value === group)?.label}. {HINTS[group]} Groups are inside each section below,
    ordered like Sort by.
  </p>
{/if}
{#if mode === 'sections' && summary.length}
  <nav class="summary" aria-label="Sections">
    {#each summary as x (x.id)}
      <button
        type="button"
        class="pill"
        class:warn={x.warn && x.n > 0}
        class:zero={x.n === 0}
        onclick={() => jump(x.id)}><b>{x.n}</b> {x.label}</button
      >
    {/each}
  </nav>
{/if}
{#if mode === 'calendar'}
  <CalendarView {queryText} />
{:else if mode !== 'sections'}
  <QueryBlock text={queryText} forcedView={mode} />
{:else if data}
  {#if allEmpty}<p class="empty">Nothing to do. Every task is done or has no match for these filters.</p>{/if}
  <section data-section="overdue" class:none={data.overdue.length === 0}>
    <h2 class:warn={data.overdue.length > 0}>Overdue <span class="n">{data.overdue.length}</span></h2>
    {@render items(data.overdue)}
  </section>
  <section data-section="today" class:none={data.today_items.length === 0}>
    <h2>Today <span class="n">{data.today_items.length}</span></h2>
    {@render items(data.today_items)}
  </section>
  <section data-section="upcoming" class:none={data.upcoming.every((d) => d.items.length === 0)}>
    <h2>Next 7 days <span class="n">{data.upcoming.reduce((n, d) => n + d.items.length, 0)}</span></h2>
    {#each data.upcoming as day (day.date)}
      <h3 data-day={day.date}>{shownDate(day.date)}</h3>
      {@render items(day.items)}
    {/each}
  </section>
  <section data-section="later" class:none={data.later.length === 0}>
    <h2>Later <span class="n">{data.later.length}</span></h2>
    {@render items(data.later)}
  </section>
  <section data-section="undated" class:none={data.undated.length === 0}>
    <h2>No date <span class="n">{data.undated.length}</span></h2>
    {@render items(data.undated)}
  </section>
{/if}

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.6em 1.2em;
    margin-bottom: 0.9em;
  }
  .head h1 {
    margin: 0;
  }
  /* segmented control: joined buttons, the chosen one filled */
  .modes {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    max-width: 100%;
    overflow-x: auto;
  }
  .mode {
    border: 0;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    padding: 0.35em 0.85em;
    min-height: 2.2em;
    cursor: pointer;
    text-transform: capitalize;
    font-size: 0.9em;
  }
  .mode:last-child {
    border-right: 0;
  }
  .mode:hover {
    background: var(--chip);
    color: var(--text);
  }
  .mode.on {
    background: var(--selection);
    color: var(--text);
    font-weight: 600;
  }
  .mode:focus-visible {
    outline: 2px solid var(--accent, currentColor);
    outline-offset: -2px;
  }
  /* the filters are one toolbar: a small caption above each control, all the same height */
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.7em 1em;
    padding: 0.7em 0.9em;
    margin-bottom: 0.9em;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--chip);
    font-size: 0.9em;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2em;
    flex: 1 1 11em;
    min-width: 8em;
  }
  .field.narrow {
    flex: 0 1 8em;
  }
  /* the suggestion input is a child component: its own style does not reach here */
  .field :global(input) {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
  }
  .cap {
    font-size: 0.75em;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .filters select {
    min-height: 2.2em;
  }
  .done {
    display: flex;
    align-items: center;
    gap: 0.4em;
    min-height: 2.2em;
  }
  .clear {
    margin-left: auto;
    min-height: 2.2em;
    border: 0;
    background: transparent;
    color: var(--link, var(--muted));
    text-decoration: underline;
    cursor: pointer;
  }
  /* counts per section; a red one is overdue work, a faded one is empty */
  .summary {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4em;
    margin-bottom: 0.4em;
  }
  .pill {
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    border-radius: 999px;
    padding: 0.25em 0.8em;
    min-height: 2em;
    cursor: pointer;
    font-size: 0.85em;
  }
  .pill.zero {
    color: var(--muted);
  }
  .pill.warn {
    color: var(--danger);
    border-color: var(--danger);
  }
  .pill:hover {
    background: var(--chip);
  }
  h2 {
    font-size: 1.05em;
    border-bottom: 1px solid var(--border);
    margin-top: 1.6em;
    padding-bottom: 0.25em;
    scroll-margin-top: 0.5em;
  }
  h2.warn {
    color: var(--danger);
  }
  /* an empty section is a quiet line, not a heading with space around it */
  section.none h2 {
    font-size: 0.9em;
    font-weight: 500;
    color: var(--muted);
    margin-top: 0.9em;
    border-bottom-style: dashed;
  }
  .empty {
    color: var(--muted);
    text-align: center;
    margin: 1.5em 0;
  }
  /* a group heading: a bar the eye can stop at, with the page as a link */
  h4.group {
    display: flex;
    align-items: center;
    gap: 0.5em;
    font-size: 0.9em;
    font-weight: 600;
    margin: 1.1em 0 0.15em;
    padding: 0.3em 0.6em;
    border-left: 3px solid var(--accent);
    background: var(--chip);
    border-radius: 0 6px 6px 0;
  }
  h4.group a {
    color: inherit;
    text-decoration: none;
  }
  h4.group a:hover {
    text-decoration: underline;
    color: var(--link);
  }
  .hint {
    margin: 0 0 0.6em;
    font-size: 0.8em;
    color: var(--muted);
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
  @media (max-width: 600px) {
    .field,
    .field.narrow {
      flex: 1 1 40%;
    }
    .clear {
      margin-left: 0;
    }
  }
</style>
