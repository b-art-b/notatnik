<script lang="ts">
  // The list behind `/query`: a filter box and the example queries, each with a line on what it does. Arrow keys
  // move, Enter inserts the query as a query block, Escape closes. It takes the focus for the filter box; the parent
  // decides where the focus goes after pick/cancel (like TemplatePicker).
  import { onMount } from 'svelte'
  import { QUERY_EXAMPLES, type QueryExample } from '../queryExamples'
  import { navigate } from '../stores/app.svelte'
  import { openSettingsSection } from '../stores/settingsParts.svelte'

  let { onpick, oncancel }: { onpick: (query: string) => void; oncancel: () => void } = $props()

  let q = $state('')
  // the position in the filtered list; `$state` so the highlight follows it
  let active = $state(0)
  let input: HTMLInputElement
  // `$derived` recomputes when `q` changes. It only reads state; writing `active` happens in `oninput` below
  const shown = $derived(
    QUERY_EXAMPLES.filter((e) =>
      `${e.title} ${e.description} ${e.query}`.toLowerCase().includes(q.trim().toLowerCase()),
    ),
  )

  onMount(() => input.focus())

  function key(e: KeyboardEvent) {
    e.stopPropagation() // keep Enter/Escape/arrows away from the editor and the global handler
    if (e.key === 'Escape') {
      e.preventDefault()
      oncancel()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      if (shown.length) active = (active + (e.key === 'ArrowDown' ? 1 : -1) + shown.length) % shown.length
    } else if (e.key === 'Enter' && shown[active]) {
      e.preventDefault()
      onpick(shown[active].query)
    }
  }
  const isActive = (e: QueryExample) => shown[active]?.id === e.id

  /** The whole list, with the queries written out, is in Settings → Templates. */
  function browse() {
    oncancel()
    openSettingsSection('templates', 'example-queries')
    navigate({ kind: 'settings' })
  }
</script>

<!-- a click inside must not reach the block's own click handler (it would re-open the editor) -->
<div
  class="example-picker"
  role="dialog"
  aria-label="Pick an example query"
  tabindex="-1"
  data-testid="query-example-picker"
  onkeydown={key}
  onclick={(e) => e.stopPropagation()}
>
  <input
    bind:this={input}
    bind:value={q}
    oninput={() => (active = 0)}
    placeholder="Filter examples…"
    aria-label="Filter examples"
  />
  <div class="list" role="listbox" aria-label="Example queries">
    {#each shown as e (e.id)}
      <button
        type="button"
        role="option"
        aria-selected={isActive(e)}
        class:active={isActive(e)}
        onclick={() => onpick(e.query)}
      >
        <span class="title">{e.title}</span>
        <code class="preview">{e.query.split('\n').join(' · ')}</code>
      </button>
    {:else}
      <p class="none">No example matches.</p>
    {/each}
  </div>
  <button type="button" class="browse" data-testid="browse-examples" onclick={browse}>
    Browse all examples in Settings →
  </button>
</div>

<style>
  .example-picker {
    display: flex;
    flex-direction: column;
    gap: 0.4em;
    padding: 0.6em;
    width: 28em;
    max-width: 90vw;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow:
      0 8px 28px rgb(0 0 0 / 0.18),
      0 1px 3px rgb(0 0 0 / 0.1);
    font-size: 0.9em;
  }
  .list {
    max-height: 19em;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .list button {
    display: flex;
    flex-direction: column;
    gap: 0.1em;
  }
  button {
    border: none;
    background: none;
    text-align: left;
    padding: 0.35em 0.5em;
    border-radius: 6px;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  button:hover,
  button.active {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .preview {
    font-size: 0.8em;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .browse {
    color: var(--accent);
    border-top: 1px solid var(--border);
    border-radius: 0 0 6px 6px;
    padding-top: 0.5em;
  }
  .none {
    margin: 0.4em 0;
    color: var(--muted);
  }
</style>
