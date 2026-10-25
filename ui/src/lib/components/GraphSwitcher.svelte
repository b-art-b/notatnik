<script lang="ts">
  // The name of the open graph at the top of the sidebar; a click lists the other graphs of this device to switch
  // to, and leads to the start screen to add one or to see them all.
  import { backend, type GraphHistoryEntry } from '../api'
  import { app, errorMessage, openGraph, toast } from '../stores/app.svelte'
  import { encryption, graphPadlock, refreshEncryption } from '../stores/encryption.svelte'
  import { goToSettings } from '../stores/settingsParts.svelte'
  import { GRAPH_COLOR_NAMES, graphColorName, graphColorVar } from '../graphColor'
  import { openBeside, refreshWorkspace } from '../stores/workspace.svelte'

  // what the graph encrypts by default (see `graphPadlock`); read again whenever another graph is open
  const padlock = $derived(graphPadlock(encryption.status))
  $effect(() => {
    void app.graph?.id
    void refreshEncryption()
  })

  let open = $state(false)
  let graphs = $state<GraphHistoryEntry[]>([])
  // the wrapper, to tell clicks inside the menu from clicks outside
  let box: HTMLDivElement | undefined = $state()

  /** Only the graphs that can be switched to: not the open one, folder still there. */
  const others = $derived(graphs.filter((g) => g.id !== app.graph?.id && g.exists))

  async function toggle() {
    open = !open
    if (!open) return
    try {
      graphs = await backend.call('list_graphs', {})
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  async function showBeside(g: GraphHistoryEntry) {
    open = false
    await openBeside(g.path)
  }

  async function switchTo(g: GraphHistoryEntry) {
    open = false
    await openGraph(g.path, false)
  }

  let colors = $state(false)
  async function pickColor(color: number | null) {
    if (!app.graph) return
    try {
      graphs = await backend.call('graph_color_set', { id: app.graph.id, color })
      app.graph = await backend.call('graph_info', {})
      void refreshWorkspace()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  function encryptionSettings() {
    open = false
    goToSettings('sync', 'encryption')
    location.hash = '#/settings'
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
    title={app.graph?.root}
    onclick={toggle}
  >
    <span class="gdot" data-testid="graph-dot" aria-hidden="true"></span>
    <span class="name">{app.graph?.name ?? 'Graph'}</span>
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
        {#each others as g (g.id)}
          <div class="entry">
            <button type="button" role="menuitem" class="item" title={g.path} onclick={() => switchTo(g)}
              ><span class="gdot" style:background={graphColorVar(g.color)} aria-hidden="true"
              ></span>{g.name}{#if g.remote_url}<span class="cloud" aria-hidden="true"> ☁</span>{/if}</button
            >
            {#if backend.slot === ''}
              <button
                type="button"
                role="menuitem"
                class="beside-btn"
                data-testid="graph-beside"
                title={`Show ${g.name} next to this graph`}
                aria-label={`Open ${g.name} beside`}
                onclick={() => showBeside(g)}>Beside</button
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
              aria-pressed={app.graph?.color === i}
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
          {app.graph ? graphColorName(app.graph.color) : ''}
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
  .entry {
    display: flex;
    align-items: center;
    gap: 0.2em;
  }
  .beside-btn {
    flex: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font-size: 0.8em;
    padding: 0.1em 0.5em;
    cursor: pointer;
  }
  .beside-btn:hover,
  .beside-btn:focus-visible {
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
