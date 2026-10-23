<script lang="ts">
  // All pages: a namespace tree, or a flat list you can filter and sort. Svelte idioms: `$state` holds what
  // the user changes, `$derived` the filtered/sorted rows computed from it (never written to).
  import { backend, type PageListRow } from '../api'
  import { bus } from '../stores/events.svelte'
  import { pageHash } from '../route'
  import { isArchived, isArchivedItself } from '../stores/archived.svelte'
  import { toggleArchive } from '../pageActions'
  import DeleteDialog from './DeleteDialog.svelte'
  import { encryption, padlockOf, refreshEncryption } from '../stores/encryption.svelte'
  import Padlock from './Padlock.svelte'

  // the padlock: only where the file differs from the graph's default (encrypted when only marked paths are,
  // plain when everything is)
  const padlock = (r: PageListRow) => padlockOf(encryption.status, r.path ?? '')
  $effect(() => {
    void refreshEncryption()
  })
  import NamespaceTree from './NamespaceTree.svelte'

  type SortKey = 'name' | 'blocks' | 'backlinks' | 'modified'
  const SHOWN = 300 // rows rendered before "Show more" (10,000 rows would make the page sluggish)

  let rows = $state<PageListRow[]>([])
  let loaded = $state(false)
  let mode = $state<'tree' | 'list'>('list')
  let filter = $state('')
  let sortKey = $state<SortKey>('name')
  let desc = $state(false)
  let journals = $state(false)
  let showArchived = $state(false)
  let onlyEncrypted = $state(false)
  let virtual = $state(true)
  let limit = $state(SHOWN)

  try {
    const s = JSON.parse(localStorage.getItem('notatnik-all-pages') ?? '{}')
    if (s.mode === 'tree' || s.mode === 'list') mode = s.mode
    if (typeof s.journals === 'boolean') journals = s.journals
    if (typeof s.virtual === 'boolean') virtual = s.virtual
  } catch {
    /* defaults */
  }
  $effect(() => {
    try {
      localStorage.setItem('notatnik-all-pages', JSON.stringify({ mode, journals, virtual }))
    } catch {
      /* not remembered */
    }
  })

  // the row whose "Delete…" confirmation is open (a page name), if any
  let deleting = $state('')

  $effect(() => {
    void bus.version
    let cancelled = false
    backend.call('all_pages', {}).then((r) => {
      if (!cancelled) {
        rows = r
        loaded = true
      }
    })
    return () => {
      cancelled = true
    }
  })

  // the Namespaces view with the padlock filter: the pages with a padlock and the names above them (lowercase, like page keys)
  const encryptedNames = $derived.by(() => {
    const out = new Set<string>()
    for (const r of rows) {
      if (!padlock(r)) continue
      const parts = r.name.toLowerCase().split('/')
      for (let i = 1; i <= parts.length; i++) out.add(parts.slice(0, i).join('/'))
    }
    return out
  })

  const visible = $derived.by(() => {
    const q = filter.trim().toLowerCase()
    const out = rows.filter(
      (r) =>
        (journals || !r.is_journal) &&
        (virtual || !r.is_virtual) &&
        (showArchived || !isArchived(r.name)) &&
        (!onlyEncrypted || !!padlock(r)) &&
        (!q || r.name.toLowerCase().includes(q)),
    )
    const dir = desc ? -1 : 1
    const cmp = (a: PageListRow, b: PageListRow): number => {
      switch (sortKey) {
        case 'blocks':
          return a.blocks - b.blocks
        case 'backlinks':
          return a.backlinks - b.backlinks
        case 'modified':
          return (a.modified ?? '').localeCompare(b.modified ?? '')
        default:
          return a.name.toLowerCase().localeCompare(b.name.toLowerCase())
      }
    }
    return out.sort((a, b) => dir * cmp(a, b) || a.name.localeCompare(b.name))
  })

  function sortBy(key: SortKey) {
    if (sortKey === key) desc = !desc
    else {
      sortKey = key
      // numbers and dates are most useful biggest/newest first
      desc = key !== 'name'
    }
    limit = SHOWN
  }
  const arrow = (key: SortKey) => (sortKey === key ? (desc ? ' ▾' : ' ▴') : '')
  const ariaSort = (key: SortKey) =>
    sortKey === key ? (desc ? 'descending' : 'ascending') : ('none' as const)

  // show the filter result from the top again whenever the filter or toggles change
  $effect(() => {
    void filter
    void journals
    void showArchived
    void onlyEncrypted
    void virtual
    limit = SHOWN
  })
  import PageIcon from './PageIcon.svelte'
</script>

<h1>All pages <span class="count" data-testid="pages-count">{visible.length}</span></h1>

<div class="bar">
  <div class="seg" role="group" aria-label="View">
    <button class:on={mode === 'list'} aria-pressed={mode === 'list'} onclick={() => (mode = 'list')}
      >List</button
    >
    <button class:on={mode === 'tree'} aria-pressed={mode === 'tree'} onclick={() => (mode = 'tree')}
      >Namespaces</button
    >
  </div>
  {#if mode === 'list'}
    <input
      class="filter"
      type="search"
      placeholder="Filter pages…"
      aria-label="Filter pages"
      bind:value={filter}
    />
    <label><input type="checkbox" bind:checked={journals} /> Journals</label>
    <label><input type="checkbox" bind:checked={virtual} /> Pages without a file</label>
    <label><input type="checkbox" bind:checked={showArchived} data-testid="show-archived" /> Archived</label>
  {/if}
  {#if encryption.status && (encryption.status.scope === 'all' || encryption.status.paths.length)}
    <label
      ><input type="checkbox" bind:checked={onlyEncrypted} data-testid="only-encrypted" />
      {encryption.status.scope === 'all' ? 'Only plain on the server' : 'Only encrypted'}</label
    >
  {/if}
</div>

{#if mode === 'tree'}
  <div class="tree" data-testid="pages-tree">
    <NamespaceTree actions lock only={onlyEncrypted ? encryptedNames : null} />
  </div>
{:else if !loaded}
  <p class="muted">Loading…</p>
{:else if visible.length === 0}
  <p class="muted">No pages match.</p>
{:else}
  <table data-testid="pages-table">
    <thead>
      <tr>
        <th scope="col" aria-sort={ariaSort('name')}
          ><button onclick={() => sortBy('name')}>Page{arrow('name')}</button></th
        >
        <th scope="col" class="num" aria-sort={ariaSort('blocks')}
          ><button onclick={() => sortBy('blocks')}>Blocks{arrow('blocks')}</button></th
        >
        <th scope="col" class="num" aria-sort={ariaSort('backlinks')}
          ><button onclick={() => sortBy('backlinks')}>Linked from{arrow('backlinks')}</button></th
        >
        <th scope="col" aria-sort={ariaSort('modified')}
          ><button onclick={() => sortBy('modified')}>Modified{arrow('modified')}</button></th
        >
        <th scope="col"><span class="sr-only">Actions</span></th>
      </tr>
    </thead>
    <tbody>
      {#each visible.slice(0, limit) as r (r.name)}
        <tr>
          <td>
            <a href={pageHash(r.name)} class:virtual={r.is_virtual} data-page={r.name}
              ><PageIcon name={r.name} align />{r.name}</a
            >
            {#if r.is_virtual}<span class="tag">no file</span>{/if}
            <Padlock mark={padlock(r)} testid="row-lock" size="0.85em" />
          </td>
          <td class="num">{r.is_virtual ? '' : r.blocks}</td>
          <td class="num">{r.backlinks || ''}</td>
          <td class="muted">{r.modified ?? ''}</td>
          <td class="acts">
            <!-- a page without a file has nothing to archive or delete; a journal day is neither -->
            {#if !r.is_virtual && !r.is_journal}
              {#if isArchivedItself(r.name)}
                <button class="act" data-testid="row-archive" onclick={() => toggleArchive(r.name)}
                  >Unarchive</button
                >
              {:else if !isArchived(r.name)}
                <button
                  class="act"
                  data-testid="row-archive"
                  title="Hide this page (and the pages below it) from the sidebar and the agenda"
                  onclick={() => toggleArchive(r.name)}>Archive</button
                >
              {/if}
              <button
                class="act danger"
                data-testid="row-delete"
                aria-label={`Delete ${r.name}`}
                onclick={() => (deleting = deleting === r.name ? '' : r.name)}>Delete…</button
              >
            {/if}
          </td>
        </tr>
        {#if deleting === r.name}
          <tr class="confirm">
            <td colspan="5"><DeleteDialog page={r.name} stay onclose={() => (deleting = '')} /></td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
  {#if visible.length > limit}
    <p>
      Showing {limit} of {visible.length}.
      <button onclick={() => (limit += SHOWN)}>Show more</button>
      <button onclick={() => (limit = visible.length)}>Show all</button>
    </p>
  {/if}
{/if}

<style>
  .count {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.5em;
    vertical-align: middle;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.8em;
    margin: 0.5em 0 1em;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 7px;
    overflow: hidden;
  }
  .seg button {
    border: none;
    background: none;
    color: inherit;
    padding: 0.3em 0.9em;
    cursor: pointer;
  }
  .seg button.on {
    background: var(--selection);
    font-weight: 600;
  }
  .filter {
    min-width: 14em;
    flex: 1;
    max-width: 24em;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 0.3em 0.6em;
    border-bottom: 1px solid var(--border);
  }
  th button {
    border: none;
    background: none;
    color: var(--muted);
    font-weight: 600;
    cursor: pointer;
    padding: 0;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  a.virtual {
    font-style: italic;
    color: var(--muted);
  }
  .tag {
    font-size: 0.72em;
    color: var(--muted);
    background: var(--chip);
    border-radius: 6px;
    padding: 0 0.4em;
    margin-left: 0.4em;
  }
  .tree {
    max-width: 40em;
  }
  .acts {
    text-align: right;
    white-space: nowrap;
  }
  .act {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0.1em 0.4em;
    font: inherit;
    font-size: 0.85em;
  }
  .act:hover,
  .act:focus-visible {
    color: var(--accent);
    background: var(--chip);
    border-radius: 4px;
  }
  .act.danger:hover,
  .act.danger:focus-visible {
    color: var(--danger);
  }
  .confirm td {
    background: var(--panel);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
