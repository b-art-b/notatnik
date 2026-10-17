<script lang="ts">
  // The tasks that belong to a page, in a section between its content and its references:
  //  - "On this page": the page's own open tasks in one list (also the ones under collapsed blocks)
  //  - "In subpages": for a namespace page, the tasks of the pages below it
  //  - "Mentioning this page": tasks elsewhere whose block links to the page (a client, a person, a project)
  // It is built from three ordinary queries (page: / ns: / links:), so it follows every edit like a query block.
  // The section is hidden while there is nothing to show.
  import { backend, type ResultRow } from '../api'
  import { bus } from '../stores/events.svelte'
  import { crumbText } from '../agenda'
  import SourcedTask from './SourcedTask.svelte'

  // `page`: the tasks of a page. `segment`: the tasks of every page that has this name somewhere in its path.
  // `count`: how many tasks are listed, for the badge of a tab that shows this component
  let {
    page = '',
    segment = '',
    count = $bindable(0),
  }: { page?: string; segment?: string; count?: number } = $props()

  const OPEN = 'todo, later, doing, now, waiting'
  const ALL = 'todo, later, doing, now, waiting, done, canceled'
  let found = $state<{ id: string; title: string; rows: ResultRow[] }[]>([])
  let open = $state(true)
  let showDone = $state(false)
  $effect(() => {
    count = found.reduce((n, g) => n + g.rows.length, 0)
  })

  try {
    const s = JSON.parse(localStorage.getItem('notatnik-page-tasks') ?? '{}')
    if (typeof s.open === 'boolean') open = s.open
    if (typeof s.showDone === 'boolean') showDone = s.showDone
  } catch {
    /* defaults */
  }
  $effect(() => {
    try {
      localStorage.setItem('notatnik-page-tasks', JSON.stringify({ open, showDone }))
    } catch {
      /* not remembered */
    }
  })

  // a name inside a query clause must not contain a comma or a line break
  const safe = (n: string) => !/[,\n]/.test(n)

  $effect(() => {
    void bus.version
    const name = page
    const seg = segment
    const states = showDone ? ALL : OPEN
    let cancelled = false
    const run = async (text: string): Promise<ResultRow[]> => {
      try {
        const r = await backend.call('run_query', { text, view: 'list', page: null })
        return r.errors.length ? [] : r.rows
      } catch {
        return []
      }
    }
    const timer = setTimeout(async () => {
      const queries = seg
        ? [
            {
              id: 'segment',
              title: `In pages with “${seg}” in the path`,
              text: `task: ${states}\npath: ${seg}\nview: list`,
            },
          ]
        : [
            { id: 'own', title: 'On this page', text: `task: ${states}\npage: ${name}\nview: list` },
            {
              id: 'sub',
              title: 'In subpages',
              text: `task: ${states}\nns: ${name}\n-page: ${name}\nview: list`,
            },
            {
              id: 'mentions',
              title: 'Mentioning this page',
              text: `task: ${states}\nlinks: ${name}\n-ns: ${name}\nview: list`,
            },
          ]
      if (!safe(seg || name)) return
      const rows = await Promise.all(queries.map((q) => run(q.text)))
      if (!cancelled) found = queries.map((q, i) => ({ id: q.id, title: q.title, rows: rows[i] }))
    }, 80)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  })

  const groups = $derived(found.filter((g) => g.rows.length > 0))
  const total = $derived(found.reduce((n, g) => n + g.rows.length, 0))
</script>

{#if total > 0 || showDone}
  <section class="tasks" aria-label="Tasks" data-testid="page-tasks">
    <div class="bar">
      <button class="head" onclick={() => (open = !open)} aria-expanded={open}
        >{open ? '▾' : '▸'} Tasks <span class="count" data-testid="page-tasks-count">{total}</span></button
      >
      {#if open}
        <label class="done"
          ><input type="checkbox" bind:checked={showDone} data-testid="page-tasks-done" /> Show done</label
        >
      {/if}
    </div>
    {#if open}
      {#each groups as g (g.id)}
        <div class="group" data-group={g.id}>
          <h3>{g.title} <span class="count">{g.rows.length}</span></h3>
          <div>
            {#each g.rows as r (r.id)}
              {#if r.block}
                <!-- own tasks: the chip is the parent blocks (the page is known) and shows the task in place;
                     other groups: the source note, shortened below this page -->
                <SourcedTask
                  block={r.block}
                  page={r.page}
                  breadcrumbs={r.breadcrumbs}
                  viewKey={`ptasks-${g.id}-${r.id}`}
                  relativeTo={g.id === 'sub' ? page : ''}
                  chip={g.id === 'own' ? crumbText(r.breadcrumbs) || null : undefined}
                  tip={g.id === 'own' ? 'Show this task in the page' : undefined}
                  itemTestid="page-task"
                  sourceTestid="page-task-source"
                />
              {/if}
            {/each}
          </div>
        </div>
      {/each}
      {#if total === 0}<p class="muted">No tasks.</p>{/if}
    {/if}
  </section>
{/if}

<style>
  .tasks {
    margin-top: 2em;
    border-top: 1px solid var(--border);
    padding-top: 0.6em;
  }
  .bar {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1em;
  }
  .head {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    font-weight: 600;
    padding: 0;
  }
  .done {
    font-size: 0.8em;
    color: var(--muted);
  }
  .count {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.85em;
    font-weight: 400;
  }
  h3 {
    font-size: 0.85em;
    color: var(--muted);
    margin: 1em 0 0.2em;
  }
</style>
