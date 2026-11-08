<script lang="ts">
  // The local graph (LN-9): the page, the pages it links to and the pages that link to it, one or two steps out, drawn as
  // a picture you can click and listed as text below it. Svelte idioms: `$state` is what the user changes (depth,
  // namespace) or what was fetched; `$derived` is computed from it and never written to; the `$effect` fetches again when
  // the page, the depth or the graph change, and its cleanup function drops an answer that came too late.
  import { backend, type LocalGraphView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { pageHash } from '../route'
  import { layoutGraph, shortName } from '../graphLayout'

  let { page, onclose }: { page: string | null; onclose: () => void } = $props()

  let depth = $state<1 | 2>(1)
  let ns = $state('')
  let graph = $state<LocalGraphView | null>(null)

  $effect(() => {
    void bus.version // the graph changed on disk or by an edit
    const name = page
    const d = depth
    if (name === null) {
      graph = null
      return
    }
    let stale = false
    void backend.call('local_graph', { page: name, depth: d }).then((g) => {
      if (!stale) graph = g
    })
    return () => {
      stale = true // a newer request is on its way: this answer is dropped
    }
  })

  // the namespace field leaves out the pages outside it (the page itself always stays)
  const inNs = (name: string) => {
    const n = ns.trim().replace(/\/+$/, '').toLowerCase()
    if (!n) return true
    const k = name.toLowerCase()
    return k === n || k.startsWith(`${n}/`)
  }
  const shown = $derived(graph ? graph.nodes.filter((n) => n.depth === 0 || inNs(n.name)) : [])
  const names = $derived(new Set(shown.map((n) => n.name)))
  const edges = $derived(graph ? graph.edges.filter((e) => names.has(e.from) && names.has(e.to)) : [])
  const layout = $derived(layoutGraph(shown, edges))
  const center = $derived(graph?.center ?? '')

  const linksTo = $derived(
    shown.filter((n) => n.depth > 0 && edges.some((e) => e.from === center && e.to === n.name)),
  )
  const linkedFrom = $derived(
    shown.filter((n) => n.depth > 0 && edges.some((e) => e.from === n.name && e.to === center)),
  )
  const further = $derived(
    shown.filter((n) => n.depth > 0 && !linksTo.includes(n) && !linkedFrom.includes(n)),
  )
  const alone = $derived(shown.length <= 1 && ns.trim() === '')

  // arrows end at the edge of the circle, not in its middle
  const R = 6
  const line = (from: string, to: string) => {
    const a = layout.nodes.find((n) => n.name === from)
    const b = layout.nodes.find((n) => n.name === to)
    if (!a || !b) return null
    const dx = b.x - a.x
    const dy = b.y - a.y
    const len = Math.hypot(dx, dy) || 1
    return {
      x1: a.x + (dx / len) * R,
      y1: a.y + (dy / len) * R,
      x2: b.x - (dx / len) * (R + 3),
      y2: b.y - (dy / len) * (R + 3),
    }
  }
  const anchor = (angle: number) =>
    Math.cos(angle) > 0.3 ? 'start' : Math.cos(angle) < -0.3 ? 'end' : 'middle'
</script>

<section class="graph" aria-label="Local graph" data-testid="local-graph">
  <header>
    <h2>Local graph</h2>
    <button
      type="button"
      class="close"
      aria-label="Close the graph"
      title="Close the graph"
      data-testid="graph-tab-close"
      onclick={onclose}>×</button
    >
  </header>
  {#if page === null}
    <p class="muted">Open a page to see its links.</p>
  {:else if !graph}
    <p class="muted">Loading…</p>
  {:else}
    <div class="controls">
      <div class="steps" role="group" aria-label="Steps away">
        <button type="button" class:on={depth === 1} aria-pressed={depth === 1} onclick={() => (depth = 1)}
          >1 step</button
        ><button type="button" class:on={depth === 2} aria-pressed={depth === 2} onclick={() => (depth = 2)}
          >2 steps</button
        >
      </div>
      <input
        type="text"
        bind:value={ns}
        aria-label="Only pages in the namespace"
        placeholder="Only this namespace"
      />
    </div>
    {#if alone}
      <p class="muted">This page has no links to or from other pages.</p>
    {/if}
    <svg
      viewBox="0 0 {layout.width} {layout.height}"
      role="group"
      aria-label="Pages around {center}"
      class="picture"
    >
      <defs>
        <marker
          id="arrow"
          viewBox="0 0 10 10"
          refX="9"
          refY="5"
          markerWidth="6"
          markerHeight="6"
          orient="auto"
        >
          <path d="M0 0 L10 5 L0 10 z" class="head" />
        </marker>
      </defs>
      {#each edges as e (`${e.from}>${e.to}`)}
        {@const l = line(e.from, e.to)}
        {#if l}
          <line class="edge" {...l} marker-end="url(#arrow)" />
        {/if}
      {/each}
      {#each layout.nodes as n (n.name)}
        <a
          href={pageHash(n.name)}
          aria-label={n.name}
          class="node d{n.depth}"
          class:virtual={shown.find((s) => s.name === n.name)?.is_virtual}
        >
          <title>{n.name}</title>
          <circle cx={n.x} cy={n.y} r={n.depth === 0 ? R + 3 : R} />
          <text
            x={n.x + (n.depth === 0 ? 0 : Math.cos(n.angle) * 11)}
            y={n.y + (n.depth === 0 ? 20 : Math.sin(n.angle) * 11 + 3)}
            text-anchor={n.depth === 0 ? 'middle' : anchor(n.angle)}>{shortName(n.name)}</text
          >
        </a>
      {/each}
    </svg>
    {#if graph.truncated}
      <p class="muted" role="status">Only the nearest 60 pages are shown.</p>
    {/if}
    {#if linksTo.length}
      <h3>Links to</h3>
      <ul>
        {#each linksTo as n (n.name)}<li><a href={pageHash(n.name)}>{n.name}</a></li>{/each}
      </ul>
    {/if}
    {#if linkedFrom.length}
      <h3>Linked from</h3>
      <ul>
        {#each linkedFrom as n (n.name)}<li><a href={pageHash(n.name)}>{n.name}</a></li>{/each}
      </ul>
    {/if}
    {#if further.length}
      <h3>Further away</h3>
      <ul>
        {#each further as n (n.name)}<li><a href={pageHash(n.name)}>{n.name}</a></li>{/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  .graph {
    display: flex;
    flex-direction: column;
    gap: 0.5em;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 1em;
  }
  .close {
    min-width: 32px;
    min-height: 32px;
    border: none;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 1.2em;
    cursor: pointer;
  }
  .close:hover {
    color: var(--text);
  }
  .muted {
    color: var(--muted);
    margin: 0;
  }
  .controls {
    display: flex;
    gap: 0.5em;
    align-items: center;
    flex-wrap: wrap;
  }
  .steps {
    display: flex;
  }
  .steps button {
    min-height: 32px;
    padding: 0 0.7em;
    border: 1px solid var(--border);
    background: none;
    color: var(--muted);
    font: inherit;
    cursor: pointer;
  }
  .steps button:first-child {
    border-radius: 6px 0 0 6px;
  }
  .steps button:last-child {
    border-radius: 0 6px 6px 0;
    border-left: none;
  }
  .steps button.on {
    color: var(--text);
    background: var(--chip);
    font-weight: 600;
  }
  input[type='text'] {
    flex: 1;
    min-width: 8em;
    padding: 0.35em 0.5em;
    font: inherit;
  }
  .picture {
    width: 100%;
    height: auto;
    max-height: 70vh;
  }
  .edge {
    stroke: var(--muted);
    stroke-opacity: 0.55;
    stroke-width: 1;
  }
  .head {
    fill: var(--muted);
    fill-opacity: 0.7;
  }
  .node circle {
    fill: var(--accent);
    fill-opacity: 0.85;
  }
  .node.d0 circle {
    fill: var(--accent);
    fill-opacity: 1;
  }
  .node.d2 circle {
    fill-opacity: 0.45;
  }
  .node.virtual circle {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .node text {
    font-size: 10px;
    fill: var(--text);
    pointer-events: none;
  }
  .node:hover circle,
  .node:focus-visible circle {
    stroke: var(--text);
    stroke-width: 2;
  }
  .node:focus-visible {
    outline: none;
  }
  h3 {
    margin: 0.6em 0 0.1em;
    font-size: 0.9em;
    color: var(--muted);
    font-weight: 600;
  }
  ul {
    margin: 0;
    padding-left: 1.2em;
  }
</style>
