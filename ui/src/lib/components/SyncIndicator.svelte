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
    title={sync.status.error ?? `Synced with ${sync.status.url}. Click to sync now.`}
    disabled={sync.busy || sync.status.state === 'syncing'}
    onclick={() => syncNow()}
  >
    <!-- the arrow turns while a sync runs (`spin`); the text below it is the status line -->
    <span class="ico" class:spin={sync.busy || sync.status.state === 'syncing'} aria-hidden="true">↻</span>
    <span class="txt">
      <strong>Sync now</strong>
      <small>{syncLine(sync.status)}</small>
    </span>
  </button>
{/if}

<style>
  .sync {
    display: flex;
    align-items: center;
    gap: 0.6em;
    width: calc(100% - 1.2em);
    margin: 0.2em 0.6em;
    text-align: left;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: none;
    padding: 0.4em 0.6em;
    color: var(--text);
    cursor: pointer;
  }
  .sync:hover:not(:disabled),
  .sync:focus-visible {
    background: var(--chip);
  }
  .sync:disabled {
    cursor: progress;
  }
  .ico {
    font-size: 1.2em;
    line-height: 1;
    color: var(--muted);
  }
  .spin {
    animation: turn 1s linear infinite;
  }
  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .txt strong {
    font-size: 0.85em;
    font-weight: 600;
  }
  .txt small {
    color: var(--muted);
    font-size: 0.75em;
  }
  .sync.problem {
    border-color: var(--danger);
  }
  .sync.problem .txt small,
  .sync.problem .ico {
    color: var(--danger);
  }
</style>
