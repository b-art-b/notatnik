<script lang="ts">
  // The second graph, next to the first (MG-6). It is the whole app loaded again in a frame of the same origin, so each
  // pane has its own stores, shortcuts and palette: the stores keep the state of one graph, and a second graph in the
  // same page would have shared them. The frame has no connection to the backend of its own; it asks this window
  // (`api/index.ts`), which adds the slot to every call.
  import { app, setBesideWidth } from '../stores/app.svelte'
  import { beside, BESIDE, closeBeside, known, openBeside, pane } from '../stores/workspace.svelte'
  import { graphColorVar } from '../graphColor'

  const info = $derived(beside())
  // the graphs the pane could show instead: not the one in the first pane, not the one shown
  const others = $derived(
    known.graphs.filter((g) => g.exists && g.id !== app.graph?.id && g.id !== info?.graph_id),
  )
  let menu = $state(false)
  let resizing = $state(false)

  // the frame's address: the same page with `?slot=1`; the route inside is its own (the hash)
  const src = $derived.by(() => {
    const u = new URL(window.location.href)
    u.search = `?slot=${BESIDE}`
    u.hash = '#/journals'
    return u.toString()
  })

  async function show(path: string) {
    menu = false
    await openBeside(path)
  }

  // Dragging the pane's left edge. Pointer capture keeps the events coming while the pointer is over the frame.
  function startResize(e: PointerEvent) {
    e.preventDefault()
    const handle = e.currentTarget as HTMLElement
    handle.setPointerCapture(e.pointerId)
    resizing = true
    const move = (m: PointerEvent) => setBesideWidth(window.innerWidth - m.clientX, false)
    const stop = () => {
      handle.removeEventListener('pointermove', move)
      handle.removeEventListener('pointerup', stop)
      handle.removeEventListener('pointercancel', stop)
      resizing = false
      setBesideWidth(app.besideWidth)
    }
    handle.addEventListener('pointermove', move)
    handle.addEventListener('pointerup', stop)
    handle.addEventListener('pointercancel', stop)
  }
  function resizeKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 40 : 10
    if (e.key === 'ArrowLeft') setBesideWidth(app.besideWidth + step)
    else if (e.key === 'ArrowRight') setBesideWidth(app.besideWidth - step)
    else return
    e.preventDefault()
  }
</script>

{#if info}
  <aside
    class="beside"
    class:resizing
    aria-label="Graph beside"
    data-testid="beside-pane"
    style:--pane-color={graphColorVar(info.color)}
    style:width="{app.besideWidth}px"
  >
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="grip"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize the graph beside"
      aria-valuemin={320}
      aria-valuenow={app.besideWidth}
      tabindex="0"
      data-testid="beside-resizer"
      onpointerdown={startResize}
      onkeydown={resizeKey}
    ></div>
    <header>
      <span class="gdot" aria-hidden="true"></span>
      <button
        type="button"
        class="name"
        aria-haspopup="menu"
        aria-expanded={menu}
        data-testid="beside-name"
        title={info.root}
        onclick={() => (menu = !menu)}>{info.name} <span aria-hidden="true">▾</span></button
      >
      <button
        type="button"
        class="close"
        aria-label="Close the graph beside"
        title="Close the graph beside"
        data-testid="beside-close"
        onclick={closeBeside}>✕</button
      >
      {#if menu}
        <div class="menu" role="menu" aria-label="Graph beside" data-testid="beside-menu">
          {#each others as g (g.id)}
            <button type="button" role="menuitem" title={g.path} onclick={() => show(g.path)}>
              <span class="gdot" style:background={graphColorVar(g.color)} aria-hidden="true"></span>{g.name}
            </button>
          {:else}
            <p class="none">No other graph on this device.</p>
          {/each}
        </div>
      {/if}
    </header>
    <!-- a new key starts the frame over: another graph is in the slot now -->
    {#key pane.version}
      <iframe title={`Graph beside: ${info.name}`} {src} data-testid="beside-frame"></iframe>
    {/key}
  </aside>
{/if}

<style>
  .beside {
    position: relative;
    flex: none;
    display: flex;
    flex-direction: column;
    height: 100vh;
    border-left: 1px solid var(--border);
    border-top: 3px solid var(--pane-color);
    background: var(--panel);
  }
  .beside.resizing {
    user-select: none;
  }
  /* while the edge is dragged the frame would take the pointer events */
  .beside.resizing iframe {
    pointer-events: none;
  }
  .grip {
    position: absolute;
    top: 0;
    bottom: 0;
    left: -5px;
    width: 10px;
    cursor: col-resize;
    touch-action: none;
    z-index: 40;
  }
  .grip::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 4px;
    width: 2px;
    background: transparent;
    transition: background 0.12s;
  }
  .grip:hover::after,
  .grip:focus-visible::after {
    background: var(--accent);
  }
  header {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.5em;
    padding: 0.3em 0.6em;
    border-bottom: 1px solid var(--border);
  }
  .gdot {
    display: inline-block;
    flex: none;
    width: 0.7em;
    height: 0.7em;
    border-radius: 50%;
    background: var(--pane-color);
  }
  header button {
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    border-radius: 6px;
    padding: 0.2em 0.4em;
  }
  header button:hover,
  header button:focus-visible {
    background: var(--chip);
  }
  .name {
    flex: 1;
    min-width: 0;
    text-align: left;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close {
    color: var(--muted);
  }
  .menu {
    position: absolute;
    z-index: 45;
    top: 100%;
    left: 0.4em;
    right: 0.4em;
    padding: 0.3em;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.18);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 0.5em;
    width: 100%;
    text-align: left;
  }
  .none {
    margin: 0.3em 0.5em;
    color: var(--muted);
  }
  iframe {
    flex: 1;
    min-height: 0;
    width: 100%;
    border: none;
    background: var(--bg);
  }
</style>
