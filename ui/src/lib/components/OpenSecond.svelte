<script lang="ts">
  // "+ Second graph…" under the first graph's section while only one graph is open: lists the other graphs of this device
  // and opens the one you pick as a second section of the sidebar (MG-6).
  import { graphs, openSecond } from '../stores/graphs.svelte'
  import { known } from '../stores/workspace.svelte'
  import { graphColorVar } from '../graphColor'

  let open = $state(false)
  const others = $derived(
    known.graphs.filter((e) => e.exists && !graphs.list.some((g) => g.info?.id === e.id)),
  )
</script>

{#if others.length}
  <div class="second">
    <button
      type="button"
      class="more"
      aria-expanded={open}
      data-testid="open-second"
      onclick={() => (open = !open)}>+ Second graph…</button
    >
    {#if open}
      <ul role="menu" aria-label="Open a second graph" data-testid="second-menu">
        {#each others as e (e.id)}
          <li>
            <button
              type="button"
              role="menuitem"
              title={e.path}
              onclick={() => {
                open = false
                void openSecond(e.path)
              }}
              ><span class="gdot" style:background={graphColorVar(e.color)} aria-hidden="true"
              ></span>{e.name}</button
            >
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .second {
    margin-top: 0.8em;
  }
  .more {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 0.85em;
    padding: 0.2em 0;
  }
  .more:hover {
    color: var(--link);
  }
  ul {
    list-style: none;
    margin: 0.2em 0 0;
    padding: 0;
  }
  li button {
    display: flex;
    align-items: center;
    gap: 0.45em;
    width: 100%;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    padding: 0.3em 0.4em;
    border-radius: 6px;
    cursor: pointer;
  }
  li button:hover,
  li button:focus-visible {
    background: var(--chip);
  }
  .gdot {
    flex: none;
    width: 0.7em;
    height: 0.7em;
    border-radius: 50%;
  }
</style>
