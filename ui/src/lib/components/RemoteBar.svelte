<script lang="ts">
  // A slim bar under the header of a viewer that shows a document of the graph's Nextcloud (NA-2, NA-6): it says when the
  // copy on this device is old because the server cannot be reached, and offers to keep a copy in the graph.
  import type { RemoteOpened } from '../api'
  import { keepCopy } from '../annotate/remote'

  let { info }: { info: RemoteOpened } = $props()

  let kept = $state('')
  let busy = $state(false)

  async function keep() {
    if (busy || kept) return
    busy = true
    try {
      kept = (await keepCopy(info)) ?? ''
    } finally {
      busy = false
    }
  }
</script>

<div class="bar" data-testid="remote-bar">
  <span class="where">From your Nextcloud</span>
  {#if info.offline}
    <span class="offline" role="status" data-testid="doc-offline"
      >The server cannot be reached: this is the copy of {info.fetched}.</span
    >
  {/if}
  {#if kept}
    <span class="kept" data-testid="doc-kept">A copy is kept in the graph: {kept}</span>
  {:else}
    <button
      type="button"
      onclick={keep}
      disabled={busy}
      title="Copy this document into the graph's assets, so it stays when the server does not"
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
  .kept {
    color: var(--muted);
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
