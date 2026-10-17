<script lang="ts">
  import { app } from '../stores/app.svelte'
  import ConflictResolver from './ConflictResolver.svelte'

  // the conflict copy being resolved; `null` = none open
  let open = $state<string | null>(null)
</script>

{#if app.conflicts.length}
  <div class="banner" role="alert" data-testid="conflict-banner">
    ⚠ {app.conflicts.length} sync conflict file(s):
    {#each app.conflicts as c (c)}<button class="link" onclick={() => (open = c)}>{c}</button>{/each}
  </div>
{/if}
{#if open}
  <ConflictResolver path={open} onclose={() => (open = null)} />
{/if}

<style>
  .banner {
    background: var(--warn-bg);
    padding: 0.4em 0.8em;
    border-radius: 6px;
    margin-bottom: 0.8em;
  }
</style>
