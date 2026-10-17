<script lang="ts">
  // One line in the sidebar for a graph that is kept in a server folder: how the last sync went. A click syncs now;
  // the details are in Settings → Sync. Nothing is shown for a graph that is only a local folder.
  import { onMount } from 'svelte'
  import { app } from '../stores/app.svelte'
  import { sync, syncLine, syncNow, watchSync } from '../stores/sync.svelte'

  // `onMount` runs once; the function it returns stops the polling when the component goes away
  onMount(() => watchSync())

  const problem = $derived(
    !!sync.status && (sync.status.state === 'error' || sync.status.state === 'offline'),
  )
</script>

{#if app.graph?.remote && sync.status}
  <button
    type="button"
    class="sync"
    class:problem
    data-testid="sync-status"
    data-state={sync.status.state}
    title={sync.status.error
      ? `${sync.status.error}\nClick to try again.`
      : `Click to sync now (also: Ctrl+K → Sync now).\nKept in step with ${sync.status.url}`}
    disabled={sync.busy || sync.status.state === 'syncing'}
    onclick={() => syncNow()}
  >
    <span aria-hidden="true">☁</span>
    {syncLine(sync.status)}
  </button>
{/if}

<style>
  .sync {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    padding: 0.2em 0.8em;
    color: var(--muted);
    font-size: 0.8em;
    cursor: pointer;
  }
  .sync:hover:not(:disabled) {
    color: var(--text);
  }
  .sync.problem {
    color: var(--danger);
  }
</style>
