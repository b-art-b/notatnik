<script lang="ts">
  // The head of a settings section: the title, where the settings of the section are kept (this device, or this graph
  // and so synced with it), and a short line that says what the section is for. Every section starts with one, so
  // they all read the same.
  import type { Snippet } from 'svelte'

  let { title, scope, children }: { title: string; scope?: 'device' | 'graph'; children?: Snippet } = $props()
</script>

<header class="head">
  <div class="line">
    <h2>{title}</h2>
    {#if scope}
      <span
        class="scope {scope}"
        data-testid="settings-scope"
        title={scope === 'device'
          ? 'Kept on this device only and never synced'
          : 'Kept in the graph, so it is synced with your notes'}
        >{scope === 'device' ? 'This device' : 'This graph'}</span
      >
    {/if}
  </div>
  {#if children}<p class="lead">{@render children()}</p>{/if}
</header>

<style>
  .head {
    margin: 0 0 0.6em;
  }
  .line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.4em 0.8em;
  }
  h2 {
    margin: 0.2em 0 0.1em;
    font-size: 1.45em;
  }
  .scope {
    border-radius: 999px;
    padding: 0.05em 0.7em;
    font-size: 0.78em;
    font-weight: 600;
    color: var(--muted);
    background: var(--chip);
    white-space: nowrap;
  }
  .scope.graph {
    color: var(--text);
    background: var(--selection);
  }
  .lead :global(code) {
    overflow-wrap: anywhere;
  }
  .lead {
    margin: 0.2em 0 0;
    max-width: 40em;
    color: var(--muted);
    font-size: 0.92em;
    line-height: 1.5;
  }
</style>
