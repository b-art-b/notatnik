<script lang="ts">
  // One graph's part of the sidebar (MG-6): its name and menu, Journals, Agenda, All pages, Search and Settings, the sync
  // line, Favorites, Recent, Saved queries and Pages. With two graphs open there are two of these, one above the other.
  // Everything inside reads its graph from `provideGraph`, so the second section never shows the first graph's pages.
  // A click inside a section makes its graph the *active* one, the graph the page area shows (the link carries the slot).
  import { untrack } from 'svelte'
  import { app } from '../stores/app.svelte'
  import { active } from '../stores/active.svelte'
  import { activate, closeSecond, graphs } from '../stores/graphs.svelte'
  import { overallStatus } from '../stores/pages.svelte'
  import type { GraphState } from '../stores/graphState.svelte'
  import { navigate } from '../stores/app.svelte'
  import { routeHash } from '../route'
  import { graphColorVar } from '../graphColor'
  import { provideGraph } from '../graphContext'
  import { dropOn, foreignTo } from '../transferDrag.svelte'
  import GraphSwitcher from './GraphSwitcher.svelte'
  import SyncIndicator from './SyncIndicator.svelte'
  import FavoritePages from './FavoritePages.svelte'
  import RecentPages from './RecentPages.svelte'
  import PagesPanel from './PagesPanel.svelte'

  let { g }: { g: GraphState } = $props()
  // the section is made for one graph and never gets another (a different graph is a new section)
  // svelte-ignore state_referenced_locally
  provideGraph(g)

  const isActive = $derived(active() === g)
  const many = $derived(graphs.list.length > 1)
  const route = $derived(app.route)
  // the page area shows the active graph: only that section marks the page on screen
  const here = (kind: string) => isActive && route.kind === kind
  const link = (kind: 'journals' | 'agenda' | 'pages' | 'settings') => routeHash({ kind }, g.slot)

  // the active graph's section is open; the other one folds away to its header when the person asks
  let wasActive = false
  $effect(() => {
    const now = isActive
    untrack(() => {
      if (now && !wasActive) g.setExpanded(true)
      wasActive = now
    })
  })
  const shown = $derived(!many || g.expanded)

  // something from the other graph is dragged over this section: it takes the drop (the dialog opens)
  let over = $state(false)

  /** Search belongs to the page area, which shows the active graph: make this one active first. */
  async function search() {
    if (!isActive) {
      await activate(g.slot)
      navigate({ kind: 'journals' }) // the hash follows the page area
    }
    app.searchOpen = true
  }
</script>

<section
  class="graph-section"
  class:active={isActive}
  class:many
  class:drop={over}
  data-testid="graph-section"
  data-slot={g.slot}
  aria-label={g.info?.name}
  style:--graph-color={g.info ? graphColorVar(g.info.color) : undefined}
  ondragover={(e) => {
    if (!foreignTo(g.slot)) return
    e.preventDefault() // this section takes the drop
    if (e.dataTransfer) e.dataTransfer.dropEffect = e.dataTransfer.effectAllowed === 'move' ? 'move' : 'copy'
    over = true
  }}
  ondragleave={(e) => {
    if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node | null)) over = false
  }}
  ondrop={(e) => {
    over = false
    dropOn(e, g.slot)
  }}
>
  <div class="head">
    <GraphSwitcher />
    {#if many}
      <button
        type="button"
        class="fold"
        aria-label={g.expanded ? `Fold ${g.info?.name}` : `Unfold ${g.info?.name}`}
        aria-expanded={shown}
        title={g.expanded ? 'Fold this graph' : 'Unfold this graph'}
        data-testid="section-fold"
        onclick={() => g.setExpanded(!g.expanded)}>{shown ? '▾' : '▸'}</button
      >
      {#if g.slot !== ''}
        <button
          type="button"
          class="fold"
          aria-label={`Close ${g.info?.name}`}
          title="Close this graph (the first one stays)"
          data-testid="section-close"
          onclick={closeSecond}>✕</button
        >
      {/if}
    {/if}
  </div>
  {#if shown}
    <nav>
      <a href={link('journals')} class:active={here('journals')}
        ><span class="ico ico-journals" aria-hidden="true"></span>Journals</a
      >
      <a href={link('agenda')} class:active={here('agenda')}
        ><span class="ico ico-agenda" aria-hidden="true"></span>Agenda</a
      >
      <a href={link('pages')} class:active={here('pages')}
        ><span class="ico ico-pages" aria-hidden="true"></span>All pages</a
      >
      <button class="link" onclick={search}
        ><span class="ico ico-search" aria-hidden="true"></span>Search</button
      >
      <a href={link('settings')} class:active={here('settings')}
        ><span class="ico ico-settings" aria-hidden="true"></span>Settings</a
      >
    </nav>
    {#if isActive}
      <div class="status" role="status" data-testid="save-status" data-status={overallStatus()}>
        {overallStatus() === 'saved' ? '✓ saved' : overallStatus() === 'saving' ? '… saving' : '⚠ not saved'}
      </div>
    {/if}
    <SyncIndicator />
    <FavoritePages />
    <RecentPages />
    {#if g.saved.length}
      <h3>Saved queries</h3>
      <ul class="saved">
        {#each g.saved as s (s.name)}<li>
            <a href={routeHash({ kind: 'saved', name: s.name }, g.slot)}>{s.name}</a>
          </li>{/each}
      </ul>
    {/if}
    <PagesPanel />
  {/if}
</section>

<style>
  /* The bar of the graph's color runs along the whole width of the sidebar; the second section starts with its own. */
  .graph-section {
    margin-inline: -0.8em;
    padding: 0.5em 0.8em 0.4em;
    border-top: 3px solid var(--graph-color, transparent);
  }
  .graph-section.drop {
    outline: 2px dashed var(--graph-color, var(--accent));
    outline-offset: -4px;
  }
  .graph-section.many:not(.active) {
    opacity: 0.92;
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: 0.2em;
  }
  .head :global(.switcher) {
    flex: 1;
    min-width: 0;
  }
  .fold {
    flex: none;
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0.35em 0.4em;
    border-radius: 6px;
  }
  .fold:hover,
  .fold:focus-visible {
    background: var(--chip);
    color: var(--text);
  }
</style>
