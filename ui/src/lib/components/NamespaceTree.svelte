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
  import { bus } from '../stores/events.svelte'
  import { backend, type NamespaceEntry } from '../api'
  import { pageHash } from '../route'
  import Self from './NamespaceTree.svelte'
  import { archived, isArchived } from '../stores/archived.svelte'
  import { pagesFold } from '../stores/pagesFold.svelte'

  let { parent = null }: { parent?: string | null } = $props()
  let all = $state<NamespaceEntry[]>([])
  // archived pages (and what is below them) are not in the tree
  const entries = $derived((void archived.names, all.filter((e) => !isArchived(e.name))))
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
  const isOpen = (name: string) => ns.open[name] ?? ns.all
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
        <a href={pageHash(e.name)} class:virtual={e.is_virtual} data-ns={e.name}
          ><PageIcon name={e.name} />{e.leaf}</a
        >
        {#if e.descendants}<span class="count">{e.descendants}</span>{/if}
      </span>
      {#if e.has_children && isOpen(e.name)}<Self parent={e.name} />{/if}
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
</style>
