<script lang="ts">
  // Pages grouped by the value of one page property. A group can be folded; folds are remembered.
  import type { PageGroupView } from '../api'
  import { pageHashIn } from '../route'
  import { useGraph } from '../graphContext'
  import { app } from '../stores/app.svelte'
  import { active } from '../stores/active.svelte'

  // the graph of this section (or the active one)
  const gr = useGraph()

  let { key }: { key: string } = $props()
  const current = $derived(gr === active() && app.route.kind === 'page' ? app.route.name.toLowerCase() : '')
  let loadedGroups = $state<PageGroupView[]>([])
  // archived pages are not listed; a group left without pages disappears
  const groups = $derived(
    (void gr.archived.names,
    loadedGroups
      .map((g) => ({ ...g, pages: g.pages.filter((p) => !gr.archived.is(p)) }))
      .filter((g) => g.pages.length > 0)),
  )
  let loaded = $state(false)
  // groups folded or opened by hand; the others follow `foldedByDefault` (Collapse all / Expand all)
  let folded = $state<Record<string, boolean>>({})
  let foldedByDefault = $state(true)
  const NONE = '\u0000none'

  const storeKey = (k: string) => `notatnik-pages-folded-${k}`
  const defaultKey = (k: string) => `notatnik-pages-folded-default-${k}`
  $effect(() => {
    // another property chosen: read its folds
    try {
      folded = JSON.parse(localStorage.getItem(storeKey(key)) ?? '{}')
      foldedByDefault = localStorage.getItem(defaultKey(key)) !== '0'
    } catch {
      folded = {}
      foldedByDefault = true
    }
  })

  // the buttons of the panel (requests made before this list existed are not repeated)
  const tree = gr.tree
  let seen = tree.foldN
  $effect(() => {
    if (tree.foldN === seen) return
    seen = tree.foldN
    foldedByDefault = tree.foldCollapsed
    folded = {}
    try {
      localStorage.setItem(storeKey(key), '{}')
      localStorage.setItem(defaultKey(key), foldedByDefault ? '1' : '0')
    } catch {
      /* not remembered */
    }
  })

  $effect(() => {
    void gr.bus.version
    const k = key
    let cancelled = false
    gr.backend.call('page_groups', { key: k }).then((r) => {
      if (!cancelled) {
        loadedGroups = r
        loaded = true
      }
    })
    return () => {
      cancelled = true
    }
  })

  const id = (g: PageGroupView) => g.value ?? NONE
  const isFolded = (g: PageGroupView) => folded[id(g)] ?? foldedByDefault
  function toggle(g: PageGroupView) {
    folded[id(g)] = !isFolded(g)
    try {
      localStorage.setItem(storeKey(key), JSON.stringify(folded))
    } catch {
      /* not remembered */
    }
  }
  import PageIcon from './PageIcon.svelte'
</script>

<div class="groups" data-testid="grouped-pages" aria-label={`Pages grouped by ${key}`}>
  {#if loaded && groups.length === 0}<p class="muted">No pages.</p>{/if}
  {#each groups as g (id(g))}
    <section data-group={g.value ?? ''}>
      <button class="gh" aria-expanded={!isFolded(g)} onclick={() => toggle(g)}>
        <span class="tog">{isFolded(g) ? '▸' : '▾'}</span>
        <span class="gname" class:none={g.value === null}>{g.value ?? `no ${key}`}</span>
        <span class="count">{g.pages.length}</span>
      </button>
      {#if !isFolded(g)}
        <ul>
          {#each g.pages as p (p)}
            <li>
              <a
                class="page-link"
                class:on={current === p.toLowerCase()}
                aria-current={current === p.toLowerCase() ? 'page' : undefined}
                href={pageHashIn(gr.slot, p)}
                data-page={p}
                title={p}
                ><PageIcon name={p} align /><span class="label"
                  >{#if p.includes('/')}<span class="ns">{p.slice(0, p.lastIndexOf('/') + 1)}</span
                    >{/if}{p.slice(p.lastIndexOf('/') + 1)}</span
                ></a
              >
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}
</div>

<style>
  .gh {
    display: flex;
    align-items: baseline;
    gap: 0.4em;
    width: 100%;
    border: none;
    background: none;
    color: inherit;
    font-weight: 600;
    text-align: left;
    padding: 0.2em 0;
    cursor: pointer;
  }
  .tog {
    width: 1em;
    color: var(--muted);
  }
  .gname.none {
    font-style: italic;
    color: var(--muted);
    font-weight: 500;
  }
  .count {
    margin-left: auto;
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.75em;
    font-weight: 400;
  }
  ul {
    list-style: none;
    margin: 0 0 0.4em 1.2em;
    padding: 0;
  }
  li {
    padding: 0.05em 0;
  }
  /* the folder part of a name is quiet, the page's own name is what you look for */
  .ns {
    color: var(--muted);
  }
</style>
