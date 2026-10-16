<script lang="ts">
  // "New page" (Ctrl+Alt+N, the sidebar, the palette): type a name, Enter opens the page ready for writing. A name
  // with `/` makes a namespace (`people/Anna`). The page is a file only once something is written, so cancelling
  // leaves nothing behind. Existing pages are suggested, so a page is not made twice by accident.
  import { backend, type TitleHit } from '../api'
  import { app } from '../stores/app.svelte'
  import { openPage } from '../newPage'

  let name = $state('')
  let hits = $state<TitleHit[]>([])
  let sel = $state(-1) // -1: the typed name itself
  let problem = $state('')
  let input: HTMLInputElement | undefined = $state()
  $effect(() => input?.focus())

  const exact = $derived(hits.find((h) => h.name.toLowerCase() === name.trim().toLowerCase()))

  async function typed() {
    problem = ''
    sel = -1
    hits = name.trim() ? await backend.call('search_titles', { text: name.trim(), limit: 6 }) : []
  }

  function close() {
    app.newPageOpen = false
  }

  async function open(target: string) {
    const err = await openPage(target)
    if (err) problem = err
    else close()
  }

  function enter() {
    if (sel >= 0) void open(hits[sel].name)
    else void open(exact?.name ?? name)
  }

  function key(e: KeyboardEvent) {
    e.stopPropagation()
    if (e.key === 'Escape') close()
    else if (e.key === 'ArrowDown') {
      sel = Math.min(sel + 1, hits.length - 1)
      e.preventDefault()
    } else if (e.key === 'ArrowUp') {
      sel = Math.max(sel - 1, -1)
      e.preventDefault()
    } else if (e.key === 'Enter') {
      e.preventDefault()
      enter()
    }
  }
</script>

<div class="overlay" role="presentation" onclick={close}>
  <div
    class="dialog"
    role="dialog"
    aria-label="New page"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={key}
  >
    <label for="new-page-name">New page</label>
    <input
      id="new-page-name"
      bind:this={input}
      bind:value={name}
      oninput={typed}
      placeholder="Name, or people/Anna for a page in a namespace"
      aria-label="Page name"
      aria-invalid={!!problem}
    />
    {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
    {#if name.trim() && !problem}
      <p class="hint" aria-live="polite">
        {#if exact}“{exact.name}” exists: Enter opens it.{:else}Enter creates “{name.trim()}”.{/if}
      </p>
    {/if}
    {#if hits.length}
      <ul role="listbox" aria-label="Existing pages">
        {#each hits as h, i (h.name)}
          <li role="option" aria-selected={i === sel}>
            <button type="button" class:sel={i === sel} onclick={() => void open(h.name)}
              >{h.name}<span class="detail">exists</span></button
            >
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    justify-content: center;
    padding-top: 10vh;
    z-index: 50;
  }
  .dialog {
    width: min(560px, 92vw);
    max-height: 70vh;
    overflow: auto;
    background: var(--panel);
    border-radius: 10px;
    padding: 0.8em;
    align-self: flex-start;
  }
  label {
    display: block;
    font-weight: 600;
    margin-bottom: 0.4em;
  }
  input {
    width: 100%;
    padding: 0.6em;
    box-sizing: border-box;
    font-size: 1.05em;
  }
  .hint {
    color: var(--muted);
    margin: 0.4em 0 0;
  }
  .problem {
    color: var(--danger);
    margin: 0.4em 0 0;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0.5em 0 0;
  }
  button {
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    padding: 0.4em;
    cursor: pointer;
    color: var(--text);
    display: flex;
    justify-content: space-between;
  }
  button.sel {
    background: var(--selection);
  }
  .detail {
    color: var(--muted);
    font-size: 0.8em;
  }
</style>
