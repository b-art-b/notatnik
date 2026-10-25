<script lang="ts">
  import { goToSettings } from '../stores/settingsParts.svelte'
  import { routeHash } from '../route'
  import { useGraph } from '../graphContext'
  // One line in the sidebar for a graph that is kept in a server folder: how the last sync went. A click syncs now;
  // the details are in Settings → Sync. Nothing is shown for a graph that is only a local folder.
  import { onMount } from 'svelte'
  import { syncLine } from '../stores/sync.svelte'

  // the graph of this section: its own server folder and its own state
  const g = useGraph()
  const sync = g.sync

  // `onMount` runs once; the function it returns stops the polling when the component goes away
  onMount(() => sync.watch())

  const problem = $derived(
    !!sync.status && (sync.status.state === 'error' || sync.status.state === 'offline'),
  )
</script>

{#if g.info?.remote && sync.status}
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
    onclick={() => sync.now()}
  >
    <span aria-hidden="true">☁</span>
    {syncLine(sync.status)}
  </button>
  {#if sync.status.left_alone > 0}
    <a
      class="sync warn"
      href={routeHash({ kind: 'settings' }, g.slot)}
      onclick={() => goToSettings('sync', 'encryption')}
      data-testid="sync-locked"
      title="The encryption key is locked: encrypted files are neither sent nor received. Open Settings → Sync to unlock it."
      >🔒 {sync.status.left_alone} encrypted file{sync.status.left_alone === 1 ? '' : 's'} waiting for the key</a
    >
  {/if}
  {#if sync.status.warnings.length}
    <a
      class="sync warn"
      href={routeHash({ kind: 'settings' }, g.slot)}
      onclick={() => goToSettings('sync', 'server')}
      data-testid="sync-warning"
      title={`Look reformatted by another editor:\n${sync.status.warnings.join('\n')}\nOpen Settings → Sync.`}
      >⚠ {sync.status.warnings.length} page(s) look reformatted</a
    >
  {/if}
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
  .sync.warn {
    color: var(--warn);
    text-decoration: none;
  }
  .sync.problem {
    color: var(--danger);
  }
</style>
