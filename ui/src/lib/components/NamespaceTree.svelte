<script lang="ts">
  import { bus } from '../stores/events.svelte'
  import { backend, type NamespaceEntry } from '../api'
  import { pageHash } from '../route'
  import Self from './NamespaceTree.svelte'
  import { archived, isArchived } from '../stores/archived.svelte'

  let { parent = null }: { parent?: string | null } = $props()
  let all = $state<NamespaceEntry[]>([])
  // archived pages (and what is below them) are not in the tree
  const entries = $derived((void archived.names, all.filter((e) => !isArchived(e.name))))
  let open = $state<Record<string, boolean>>({})

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
  $effect(() => {
    try {
      const saved = JSON.parse(localStorage.getItem('notatnik-ns-open') ?? '{}')
      if (parent === null) open = saved
    } catch {
      /* not remembered */
    }
  })
  function toggle(name: string) {
    open[name] = !open[name]
    try {
      localStorage.setItem('notatnik-ns-open', JSON.stringify(open))
    } catch {
      /* not remembered */
    }
  }
  import PageIcon from './PageIcon.svelte'
</script>

<ul
  role={parent === null ? 'tree' : 'group'}
  class="tree"
  aria-label={parent === null ? 'Namespaces' : undefined}
>
  {#each entries as e (e.name)}
    <li role="treeitem" aria-selected="false" aria-expanded={e.has_children ? !!open[e.name] : undefined}>
      <span class="line">
        {#if e.has_children}<button
            class="tog"
            aria-label={open[e.name] ? 'Collapse' : 'Expand'}
            onclick={() => toggle(e.name)}>{open[e.name] ? '▾' : '▸'}</button
          >{:else}<span class="tog"></span>{/if}
        <a href={pageHash(e.name)} class:virtual={e.is_virtual} data-ns={e.name}
          ><PageIcon name={e.name} />{e.leaf}</a
        >
        {#if e.descendants}<span class="count">{e.descendants}</span>{/if}
      </span>
      {#if e.has_children && open[e.name]}<Self parent={e.name} />{/if}
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
