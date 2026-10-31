<script lang="ts">
  // A ```query block rendered as live results (QV-1). Re-runs on every graph change (debounced).
  import { getContext } from 'svelte'
  import { backend, type QueryView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { app, toast, errorMessage } from '../stores/app.svelte'
  import ListView from './views/ListView.svelte'
  import TableView from './views/TableView.svelte'
  import KanbanView from './views/KanbanView.svelte'
  import TimelineView from './views/TimelineView.svelte'

  // `forcedView`: the caller picks the view (the agenda has its own switcher), so the view buttons are hidden
  let {
    text,
    saved = false,
    forcedView = null,
  }: { text: string; saved?: boolean; forcedView?: string | null } = $props()
  const queryPage = getContext<(() => string) | undefined>('queryPage')
  let viewOverride = $state<string | null>(null)
  let result = $state<QueryView | null>(null)
  let showQuery = $state(false)
  let saving = $state(false)
  let saveName = $state('')
  // not reactive on purpose: reading `result` inside the effect would make it re-run after every result
  let firstRun = true
  const id = `q${Math.random().toString(36).slice(2, 8)}`
  const view = $derived(forcedView ?? viewOverride ?? result?.view ?? 'list')

  $effect(() => {
    void bus.version
    const q = text
    const v = forcedView ?? viewOverride
    const pg = queryPage?.() ?? null
    let cancelled = false
    const timer = setTimeout(
      async () => {
        try {
          const r = await backend.call('run_query', { text: q, view: v, page: pg })
          if (!cancelled) {
            result = r
            firstRun = false
          }
        } catch (e) {
          if (!cancelled)
            result = {
              errors: [{ line: 1, column: 1, message: errorMessage(e), suggestion: null }],
              target: 'block',
              view: 'list',
              columns: [],
              group_by: null,
              expanded: null,
              rows: [],
            }
        }
      },
      firstRun ? 0 : 120,
    )
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  })

  async function save() {
    try {
      await backend.call('save_query', { name: saveName, query: text })
      toast(`Saved “${saveName}” to the sidebar`)
      saving = false
      app.savedVersion++
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  const views = ['list', 'table', 'kanban', 'timeline']
</script>

<!-- keys typed in the query UI must not reach the block's own key handling; nothing here is a control itself -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="query"
  data-testid="query-block"
  role="group"
  aria-label="Query results"
  onkeydown={(e) => e.stopPropagation()}
>
  <div class="head">
    <span class="count" data-testid="query-count"
      >{result ? `${result.rows.length} result${result.rows.length === 1 ? '' : 's'}` : '…'}</span
    >
    <!-- not a button on purpose: a click on the header (or the frame, or the shown query text) is a click on
         the block, which opens it for editing; only the results below keep their own clicks -->
    <span class="grow edit-hint"
      >{#if !forcedView && !saved}✎ click here to edit the query{/if}</span
    >
    {#if !forcedView}
      {#each views as v (v)}
        <button class="v" class:on={view === v} aria-pressed={view === v} onclick={() => (viewOverride = v)}
          >{v}</button
        >
      {/each}
    {/if}
    <button class="v" onclick={() => (showQuery = !showQuery)} aria-expanded={showQuery}>show query</button>
    {#if !saved}<button class="v" onclick={() => (saving = !saving)}>save</button>{/if}
  </div>
  {#if saving}
    <div class="save">
      <input
        aria-label="Name"
        bind:value={saveName}
        placeholder="Name in the sidebar"
        onkeydown={(e) => e.key === 'Enter' && save()}
      /><button onclick={save}>Save to sidebar</button>
    </div>
  {/if}
  {#if showQuery}
    <pre class="code">{text}</pre>
    {#if result?.expanded}
      <!-- the same query with the placeholders replaced: what actually runs on this page -->
      <div class="expanded-label">with the placeholders replaced:</div>
      <pre class="code" data-testid="query-expanded">{result.expanded}</pre>
    {/if}
  {/if}
  {#if result}
    <div class="results" role="presentation" onclick={(e) => e.stopPropagation()}>
      {#if result.errors.length}
        <ul class="errors" role="alert" data-testid="query-errors">
          {#each result.errors as e, i (i)}
            <li>line {e.line}, column {e.column}: {e.message}{e.suggestion ? `; ${e.suggestion}` : ''}</li>
          {/each}
        </ul>
      {:else if view === 'table'}
        <TableView rows={result.rows} columns={result.columns} {id} />
      {:else if view === 'kanban'}
        <KanbanView rows={result.rows} groupBy={result.group_by} />
      {:else if view === 'timeline'}
        <TimelineView rows={result.rows} />
      {:else}
        <ListView rows={result.rows} groupBy={result.group_by} {id} />
      {/if}
    </div>
  {/if}
</div>

<style>
  .query {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4em 0.6em;
    margin: 0.2em 0;
    background: var(--embed-bg);
    cursor: default;
  }
  .head {
    display: flex;
    gap: 0.3em;
    align-items: center;
    font-size: 0.8em;
  }
  .grow {
    flex: 1;
    align-self: stretch;
  }
  .edit-hint {
    color: var(--muted);
    cursor: text;
    padding: 0.2em 0;
  }
  .count {
    font-weight: 650;
  }
  .v {
    border: none;
    background: var(--chip);
    border-radius: 6px;
    padding: 0.1em 0.5em;
    cursor: pointer;
    color: var(--text);
  }
  .v.on {
    background: var(--link);
    color: var(--bg);
  }
  .errors {
    color: var(--danger);
    margin: 0.3em 0;
    padding-left: 1.2em;
  }
  .expanded-label {
    margin-top: 0.4em;
    font-size: 0.8em;
    color: var(--muted);
  }
  .code {
    background: var(--code-bg);
    padding: 0.4em;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 0.85em;
  }
</style>
