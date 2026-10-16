<script lang="ts">
  // The start screen: the graphs opened on this device (the last one first, selected), each with Open and a way to
  // remove it from the list (only the list: its folder is not touched), and a field for another folder. Also
  // reached from the palette ("Switch graph…") while a graph is open.
  import { onMount } from 'svelte'
  import { backend, type GraphHistoryEntry } from '../api'
  import { app, errorMessage, openGraph, toast } from '../stores/app.svelte'

  let path = $state('')
  let graphs = $state<GraphHistoryEntry[]>([])
  let loaded = $state(false)
  let list: HTMLUListElement | undefined = $state()

  onMount(() => {
    backend.call('list_graphs', {}).then(
      (r) => {
        graphs = r
        loaded = true
      },
      (e) => {
        loaded = true
        toast(errorMessage(e), 'error')
      },
    )
  })
  // the last graph's Open button has the focus: Enter opens it
  $effect(() => {
    void graphs
    list?.querySelector<HTMLButtonElement>('button.open:not(:disabled)')?.focus()
  })

  /** The system's folder chooser (desktop app): the folder picked is opened, or made a graph when it has none. */
  async function browse() {
    try {
      const picked = await backend.pickFolder?.()
      if (picked) await openGraph(picked, true)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  async function forget(g: GraphHistoryEntry) {
    try {
      graphs = await backend.call('forget_graph', { id: g.id })
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
</script>

<div class="welcome">
  <h1>Notatnik</h1>
  {#if graphs.length}
    <h2>Your graphs</h2>
    <ul bind:this={list} class="graphs" data-testid="graph-history" aria-label="Graphs">
      {#each graphs as g (g.id)}
        <li data-graph={g.id} class:missing={!g.exists}>
          <div class="what">
            <strong>{g.name}</strong>
            {#if g.last}<span class="badge">last opened</span>{/if}
            {#if app.graph?.id === g.id}<span class="badge">open now</span>{/if}
            <div class="path">
              {g.path}{#if !g.exists}
                · folder not found{/if}
            </div>
          </div>
          <button type="button" class="open" disabled={!g.exists} onclick={() => openGraph(g.path, false)}
            >Open</button
          >
          <button
            type="button"
            class="forget"
            aria-label={`Remove ${g.name} from the list`}
            title="Remove from this list (the folder is not touched)"
            onclick={() => forget(g)}>✕</button
          >
        </li>
      {/each}
    </ul>
  {:else if loaded}
    <p>Open a folder of notes, or create a new graph.</p>
  {/if}
  <h2>{graphs.length ? 'Another folder' : 'Open a folder'}</h2>
  <input
    bind:value={path}
    placeholder="/home/you/notes"
    aria-label="Graph folder"
    onkeydown={(e) => e.key === 'Enter' && path.trim() && openGraph(path, true)}
  />
  <button onclick={() => openGraph(path, true)} disabled={!path.trim()}>Open / create</button>
  {#if backend.pickFolder}
    <button type="button" class="browse" data-testid="browse-folder" onclick={browse}>Choose a folder…</button
    >
  {/if}
  {#if app.graph}
    <p>
      <button type="button" class="back" onclick={() => (app.needsGraph = false)}
        >← Back to {app.graph.name}</button
      >
    </p>
  {/if}
</div>

<style>
  .welcome {
    max-width: 560px;
    margin: 12vh auto;
    padding: 0 1em;
  }
  h2 {
    font-size: 1em;
    margin: 1.4em 0 0.4em;
    color: var(--muted);
    font-weight: 600;
  }
  .graphs {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.5em;
    padding: 0.5em 0.7em;
    border-bottom: 1px solid var(--border);
  }
  li:last-child {
    border-bottom: none;
  }
  li.missing {
    opacity: 0.75;
  }
  .what {
    flex: 1;
    min-width: 0;
  }
  .path {
    color: var(--muted);
    font-size: 0.85em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    margin-left: 0.4em;
    padding: 0.05em 0.5em;
    border-radius: 999px;
    background: var(--chip);
    color: var(--muted);
    font-size: 0.75em;
  }
  .forget,
  .back {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .forget:hover {
    color: var(--danger);
  }
  input {
    width: 100%;
    padding: 0.5em;
    margin-bottom: 0.5em;
    box-sizing: border-box;
  }
</style>
