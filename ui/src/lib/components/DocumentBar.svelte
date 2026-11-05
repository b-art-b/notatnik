<script lang="ts">
  // A slim bar under the header of a viewer that shows a document that is not in the graph (NA-2, NA-6, LV-6): where it is
  // from, a note when the copy is old because the server cannot be reached, and a button to keep a copy in the graph.
  import { keepCopy, type ResolvedDoc } from '../annotate/remote'

  let { doc }: { doc: ResolvedDoc } = $props()

  let kept = $state('')
  let busy = $state(false)

  async function keep() {
    if (busy || kept) return
    busy = true
    try {
      kept = (await keepCopy(doc)) ?? ''
    } finally {
      busy = false
    }
  }
</script>

<div class="bar" data-testid="remote-bar">
  <span class="where">{doc.where === 'nextcloud' ? 'From your Nextcloud' : 'On this computer'}</span>
  {#if doc.offline}
    <span class="offline" role="status" data-testid="doc-offline"
      >The server cannot be reached: this is the copy of {doc.fetched}.</span
    >
  {/if}
  {#if kept}
    <span class="kept" data-testid="doc-kept">A copy is kept in the graph: {kept}</span>
  {:else}
    <button
      type="button"
      onclick={keep}
      disabled={busy}
      title="Copy this document into the graph's assets, so it stays with the graph"
      data-testid="doc-keep">Keep a copy in the graph</button
    >
  {/if}
</div>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6em;
    padding: 0.25em 0.8em;
    border-bottom: 1px solid var(--border);
    font-size: 0.85em;
    color: var(--muted);
  }
  .offline {
    color: var(--warn);
  }
  button {
    margin-left: auto;
  }
  @media (pointer: coarse) {
    button {
      min-height: 44px;
    }
  }
</style>
