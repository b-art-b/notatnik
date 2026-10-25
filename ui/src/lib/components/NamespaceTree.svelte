<script module lang="ts">
  // shared by every level of the tree
  const KEY = 'notatnik-ns-open'
  function read(): { all: boolean; open: Record<string, boolean> } {
    try {
      const v = JSON.parse(localStorage.getItem(KEY) ?? '{}')
      // before "Expand all" the value was the map of open branches itself
      return 'open' in v ? { all: !!v.all, open: v.open ?? {} } : { all: false, open: v }
    } catch {
      return { all: false, open: {} }
    }
  }
  export const ns = $state(read())
  export function saveNs() {
    try {
      localStorage.setItem(KEY, JSON.stringify({ all: ns.all, open: ns.open }))
    } catch {
      /* not remembered */
    }
  }
</script>

<script lang="ts">
  import { endPageDrag, startPageDrag } from '../transferDrag'
  import { bus } from '../stores/events.svelte'
  import { backend, type NamespaceEntry } from '../api'
  import { pageHash } from '../route'
  import Self from './NamespaceTree.svelte'
  import { archived, isArchived } from '../stores/archived.svelte'
  import { pagesFold } from '../stores/pagesFold.svelte'
  import { encryption, padlockOf, refreshEncryption } from '../stores/encryption.svelte'
  import Padlock from './Padlock.svelte'
  import { toggleArchive } from '../pageActions'
  import DeleteDialog from './DeleteDialog.svelte'

  // `actions`: All pages shows Archive / Delete… on each page; the sidebar tree does not
  // `lock`: shows the padlock of a page that differs from the graph's default (All pages and the sidebar)
  // `only`: names to keep (lowercase): All pages with the padlock filter; every branch is then open
  let {
    parent = null,
    actions = false,
    lock = false,
    only = null,
  }: { parent?: string | null; actions?: boolean; lock?: boolean; only?: Set<string> | null } = $props()
  let deleting = $state('') // the page whose Delete… confirmation is open
  const padlock = (e: NamespaceEntry) => (actions || lock ? padlockOf(encryption.status, e.path ?? '') : null)
  $effect(() => {
    if (actions || lock) void refreshEncryption()
  })
  let all = $state<NamespaceEntry[]>([])
  // archived pages (and what is below them) are not in the tree
  const entries = $derived(
    (void archived.names,
    all.filter((e) => !isArchived(e.name) && (!only || only.has(e.name.toLowerCase())))),
  )
  // What was opened or closed by hand; every other branch follows `ns.all` (Collapse all / Expand all).
  // The state is shared by all levels of the tree, so it is read once and saved by whichever level changes it.

  $effect(() => {
    void bus.version
    let cancelled = false
    backend.call('list_namespace', { parent }).then((r) => {
      if (!cancelled) all = r
    })
    return () => {
      cancelled = true
    }
  })
  const isOpen = (name: string) => !!only || (ns.open[name] ?? ns.all)
  function toggle(name: string) {
    ns.open[name] = !isOpen(name)
    saveNs()
  }
  // the root level answers the buttons of the panel (the requests made before this tree existed are not repeated)
  let seen = pagesFold.n
  $effect(() => {
    if (parent !== null || pagesFold.n === seen) return
    seen = pagesFold.n
    ns.all = !pagesFold.collapsed
    ns.open = {}
    saveNs()
  })
  import PageIcon from './PageIcon.svelte'
</script>

<ul
  role={parent === null ? 'tree' : 'group'}
  class="tree"
  aria-label={parent === null ? 'Namespaces' : undefined}
>
  {#each entries as e (e.name)}
    <li role="treeitem" aria-selected="false" aria-expanded={e.has_children ? isOpen(e.name) : undefined}>
      <span class="line">
        {#if e.has_children}<button
            class="tog"
            aria-label={isOpen(e.name) ? 'Collapse' : 'Expand'}
            onclick={() => toggle(e.name)}>{isOpen(e.name) ? '▾' : '▸'}</button
          >{:else}<span class="tog"></span>{/if}
        <a
          href={pageHash(e.name)}
          class:virtual={e.is_virtual}
          data-ns={e.name}
          draggable={!e.is_virtual}
          ondragstart={(ev) => startPageDrag(ev, e.name, e.descendants > 0)}
          ondragend={endPageDrag}><PageIcon name={e.name} align />{e.leaf}</a
        >
        <Padlock mark={padlock(e)} testid="row-lock" />
        {#if e.descendants}<span class="count">{e.descendants}</span>{/if}
        <!-- a page without a file has nothing to archive or delete -->
        {#if actions && !e.is_virtual}
          <span class="acts">
            <button
              class="act"
              data-testid="row-archive"
              title="Hide this page (and the pages below it) from the sidebar and the agenda"
              onclick={() => toggleArchive(e.name)}>Archive</button
            >
            <button
              class="act danger"
              data-testid="row-delete"
              aria-label={`Delete ${e.name}`}
              onclick={() => (deleting = deleting === e.name ? '' : e.name)}>Delete…</button
            >
          </span>
        {/if}
      </span>
      {#if actions && deleting === e.name}
        <DeleteDialog page={e.name} stay onclose={() => (deleting = '')} />
      {/if}
      {#if e.has_children && isOpen(e.name)}<Self parent={e.name} {actions} {lock} {only} />{/if}
    </li>
  {/each}
</ul>

<style>
  .tree {
    list-style: none;
    margin: 0;
    padding-left: 0.9em;
  }
  .line {
    display: flex;
    gap: 2px;
    align-items: center;
  }
  .tog {
    width: 14px;
    border: none;
    background: none;
    color: var(--muted);
    padding: 0;
    cursor: pointer;
  }
  a {
    color: var(--text);
    text-decoration: none;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .virtual {
    color: var(--muted);
    font-style: italic;
  }
  .count {
    font-size: 0.7em;
    color: var(--muted);
  }
  .acts {
    display: inline-flex;
    gap: 0.2em;
    margin-left: 0.6em;
    white-space: nowrap;
  }
  .act {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0.1em 0.4em;
    font: inherit;
    font-size: 0.8em;
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
</style>
