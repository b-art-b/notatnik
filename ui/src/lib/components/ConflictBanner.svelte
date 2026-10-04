<script lang="ts">
  import { backend, type ConflictView } from '../api'
  import { app } from '../stores/app.svelte'
  let open = $state<ConflictView | null>(null)
  async function view(path: string) {
    open = await backend.call('read_conflict', { path })
  }
</script>

{#if app.conflicts.length}
  <div class="banner" role="alert" data-testid="conflict-banner">
    ⚠ {app.conflicts.length} sync conflict file(s):
    {#each app.conflicts as c (c)}<button class="link" onclick={() => view(c)}>{c}</button>{/each}
  </div>
{/if}
{#if open}
  <div class="conflict" role="dialog" aria-label="Conflict copy">
    <div>
      <h3>Conflict copy</h3>
      <pre>{open.text}</pre>
    </div>
    <div>
      <h3>Original {open.original ?? ''}</h3>
      <pre>{open.original_text ?? '(not found)'}</pre>
    </div>
    <button onclick={() => (open = null)}>Close</button>
  </div>
{/if}

<style>
  .banner {
    background: var(--warn-bg);
    padding: 0.4em 0.8em;
    border-radius: 6px;
    margin-bottom: 0.8em;
  }
  .conflict {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1em;
    border: 1px solid var(--border);
    padding: 0.8em;
    margin-bottom: 1em;
  }
  pre {
    white-space: pre-wrap;
    background: var(--code-bg);
    padding: 0.5em;
  }
</style>
