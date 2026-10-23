<script lang="ts">
  // The name of the open graph at the top of the sidebar; a click lists the other graphs of this device to switch
  // to, and leads to the start screen to add one or to see them all.
  import { backend, type GraphHistoryEntry } from '../api'
  import { app, errorMessage, openGraph, toast } from '../stores/app.svelte'
  import { encryption, graphPadlock, refreshEncryption } from '../stores/encryption.svelte'

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

  async function switchTo(g: GraphHistoryEntry) {
    open = false
    await openGraph(g.path, false)
  }

  function encryptionSettings() {
    open = false
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
    <span class="db" aria-hidden="true">⛁</span>
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
          <button type="button" role="menuitem" class="item" title={g.path} onclick={() => switchTo(g)}
            >{g.name}{#if g.remote_url}<span class="cloud" aria-hidden="true"> ☁</span>{/if}</button
          >
        {/each}
        <hr />
      {/if}
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
  .db,
  .caret {
    color: var(--muted);
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
  hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 0.3em 0;
  }
  .cloud {
    color: var(--muted);
  }
</style>
