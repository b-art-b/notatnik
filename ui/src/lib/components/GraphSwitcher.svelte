<script lang="ts">
  // The name of the open graph at the top of the sidebar; a click lists the other graphs of this device to switch
  // to, and leads to the start screen to add one or to see them all.
  import type { GraphHistoryEntry } from '../api'
  import { app, errorMessage, toast } from '../stores/app.svelte'
  import { active } from '../stores/active.svelte'
  import { graphs as open_graphs, openGraph, openSecond } from '../stores/graphs.svelte'
  import { graphPadlock } from '../stores/encryption.svelte'
  import { routeHash } from '../route'
  import { useGraph } from '../graphContext'
  import { goToSettings } from '../stores/settingsParts.svelte'
  import { GRAPH_COLOR_NAMES, graphColorName, graphColorVar } from '../graphColor'
  import { refreshWorkspace } from '../stores/workspace.svelte'

  // the graph of this section: its name, color and padlock; the menu switches this section to another graph
  const g = useGraph()
  // what the graph encrypts by default (see `graphPadlock`)
  const padlock = $derived(graphPadlock(g.encryption.status))
  $effect(() => {
    void g.info?.id
    void g.encryption.refresh()
  })

  let open = $state(false)
  let graphs = $state<GraphHistoryEntry[]>([])
  // the wrapper, to tell clicks inside the menu from clicks outside
  let box: HTMLDivElement | undefined = $state()

  /** Only the graphs that can be shown here: not one that is open in a section, folder still there. */
  const others = $derived(
    graphs.filter((e) => e.exists && !open_graphs.list.some((x) => x.info?.id === e.id)),
  )

  async function toggle() {
    open = !open
    if (!open) return
    try {
      graphs = await g.backend.call('list_graphs', {})
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  async function switchTo(e: GraphHistoryEntry) {
    open = false
    if (g.slot === '') await openGraph(e.path, false, '')
    else await openSecond(e.path)
  }

  /** The first section's menu can also open a graph in the second section. */
  async function asSecond(e: GraphHistoryEntry) {
    open = false
    await openSecond(e.path)
  }

  let colors = $state(false)
  async function pickColor(color: number | null) {
    if (!g.info) return
    try {
      graphs = await g.backend.call('graph_color_set', { id: g.info.id, color })
      await g.refreshInfo()
      if (g === active()) app.graph = g.info
      void refreshWorkspace()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  function encryptionSettings() {
    open = false
    goToSettings('sync', 'encryption')
    location.hash = routeHash({ kind: 'settings' }, g.slot)
  }

  function startScreen() {
    open = false
    app.needsGraph = true
  }

  function onwindowclick(e: MouseEvent) {
    if (open && box && !box.contains(e.target as Node)) open = false
  }
  function onkeydown(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      open = false
      e.stopPropagation()
    }
  }
</script>

<!-- `<svelte:window>` adds listeners to the window for as long as this component exists -->
<svelte:window onclick={onwindowclick} {onkeydown} />

<div class="switcher" bind:this={box}>
  <button
    type="button"
    class="current"
    aria-haspopup="menu"
    aria-expanded={open}
    data-testid="graph-switcher"
    title={g.info?.root}
    onclick={toggle}
  >
    <span class="gdot" data-testid="graph-dot" aria-hidden="true"></span>
    <span class="name">{g.info?.name ?? 'Graph'}</span>
    {#if padlock}<span
        class="padlock"
        class:some={padlock.kind === 'some'}
        data-testid="graph-padlock"
        data-kind={padlock.kind}
        role="img"
        aria-label={padlock.title}
        title={padlock.title}>🔒</span
      >{/if}
    <span class="caret" aria-hidden="true">▾</span>
  </button>
  {#if open}
    <div class="menu" role="menu" aria-label="Graphs" data-testid="graph-menu">
      {#if others.length}
        <div class="head">Switch to:</div>
        {#each others as e (e.id)}
          <div class="entry">
            <button type="button" role="menuitem" class="item" title={e.path} onclick={() => switchTo(e)}
              ><span class="gdot" style:background={graphColorVar(e.color)} aria-hidden="true"
              ></span>{e.name}{#if e.remote_url}<span class="cloud" aria-hidden="true"> ☁</span>{/if}</button
            >
            {#if g.slot === ''}
              <button
                type="button"
                role="menuitem"
                class="second-btn"
                data-testid="graph-second"
                title={`Show ${e.name} in a second section of the sidebar`}
                aria-label={`Open ${e.name} as the second graph`}
                onclick={() => asSecond(e)}>Second</button
              >
            {/if}
          </div>
        {/each}
        <hr />
      {/if}
      <button
        type="button"
        role="menuitem"
        class="item"
        aria-expanded={colors}
        data-testid="graph-menu-color"
        onclick={() => (colors = !colors)}>Color of this graph…</button
      >
      {#if colors}
        <div class="swatches" role="group" aria-label="Color of this graph">
          {#each GRAPH_COLOR_NAMES as n, i (n)}
            <button
              type="button"
              class="swatch"
              style:background={graphColorVar(i)}
              aria-label={n}
              aria-pressed={g.info?.color === i}
              title={n}
              data-testid={`graph-color-${i}`}
              onclick={() => pickColor(i)}
            ></button>
          {/each}
          <button type="button" class="reset" data-testid="graph-color-reset" onclick={() => pickColor(null)}
            >Automatic</button
          >
        </div>
        <p class="colorname" data-testid="graph-color-name">
          {g.info ? graphColorName(g.info.color) : ''}
        </p>
      {/if}
      <hr />
      <button type="button" role="menuitem" class="item" onclick={startScreen}>Add new graph…</button>
      <button type="button" role="menuitem" class="item" onclick={startScreen}>All graphs…</button>
      {#if padlock}
        <hr />
        <button
          type="button"
          role="menuitem"
          class="item"
          data-testid="graph-menu-encryption"
          onclick={encryptionSettings}>Encryption settings…</button
        >
      {/if}
    </div>
  {/if}
</div>

<style>
  .switcher {
    position: relative;
    margin-bottom: 0.4em;
    /* the button that hides the sidebar sits at this corner */
    margin-right: 32px;
  }
  .current {
    display: flex;
    align-items: center;
    gap: 0.5em;
    width: 100%;
    padding: 0.4em 0.6em;
    border: none;
    border-radius: 8px;
    background: var(--chip);
    color: inherit;
    font-weight: 600;
    cursor: pointer;
    text-align: left;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .padlock {
    font-size: 0.8em;
  }
  /* only some paths are encrypted: the same padlock, quieter */
  .padlock.some {
    opacity: 0.45;
  }
  .caret {
    color: var(--muted);
  }
  /* the graph's color, as a dot before its name */
  .gdot {
    display: inline-block;
    flex: none;
    width: 0.7em;
    height: 0.7em;
    margin-right: 0.45em;
    border-radius: 50%;
    background: var(--graph-color, var(--muted));
  }
  .current .gdot {
    margin-right: 0;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35em;
    padding: 0.2em 0.5em 0.3em;
  }
  .swatch {
    width: 1.3em;
    height: 1.3em;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
  }
  .swatch[aria-pressed='true'] {
    border-color: var(--text);
  }
  .reset {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 0.85em;
    text-decoration: underline;
  }
  .colorname {
    margin: 0 0.5em 0.2em;
    color: var(--muted);
    font-size: 0.85em;
  }
  .menu {
    position: absolute;
    z-index: 30;
    left: 0;
    right: 0;
    margin-top: 0.2em;
    padding: 0.3em;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.18);
  }
  .head {
    padding: 0.3em 0.5em;
    color: var(--muted);
    font-size: 0.85em;
  }
  .item {
    display: block;
    width: 100%;
    padding: 0.35em 0.5em;
    border: none;
    border-radius: 6px;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .item:hover,
  .item:focus-visible {
    background: var(--chip);
  }
  .entry .item {
    flex: 1;
    width: auto;
    min-width: 0;
  }
  .entry {
    display: flex;
    align-items: center;
    gap: 0.2em;
  }
  .second-btn {
    flex: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font-size: 0.8em;
    padding: 0.1em 0.5em;
    cursor: pointer;
  }
  .second-btn:hover,
  .second-btn:focus-visible {
    background: var(--chip);
    color: var(--text);
  }
  hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 0.3em 0;
  }
  .cloud {
    color: var(--muted);
  }
</style>
