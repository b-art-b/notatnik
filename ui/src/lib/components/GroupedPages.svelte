<script lang="ts">
  // Pages grouped by the value of one page property. A group can be folded; folds are remembered.
  import { backend, type PageGroupView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { pageHash } from '../route'
  import { archived, isArchived } from '../stores/archived.svelte'

  let { key }: { key: string } = $props()
  let loadedGroups = $state<PageGroupView[]>([])
  // archived pages are not listed; a group left without pages disappears
  const groups = $derived(
    (void archived.names,
    loadedGroups
      .map((g) => ({ ...g, pages: g.pages.filter((p) => !isArchived(p)) }))
      .filter((g) => g.pages.length > 0)),
  )
  let loaded = $state(false)
  let folded = $state<Record<string, boolean>>({})
  const NONE = '\u0000none'

  const storeKey = (k: string) => `notatnik-pages-folded-${k}`
  $effect(() => {
    // another property chosen: read its folds
    try {
      folded = JSON.parse(localStorage.getItem(storeKey(key)) ?? '{}')
    } catch {
      folded = {}
    }
  })

  $effect(() => {
    void bus.version
    const k = key
    let cancelled = false
    backend.call('page_groups', { key: k }).then((r) => {
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
  // the pages without a value start folded: they are usually the long tail
  const isFolded = (g: PageGroupView) => folded[id(g)] ?? g.value === null
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
            <li><a href={pageHash(p)} data-page={p}><PageIcon name={p} />{p}</a></li>
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
    padding: 0.1em 0;
    overflow-wrap: anywhere;
  }
  a {
    text-decoration: none;
  }
</style>
