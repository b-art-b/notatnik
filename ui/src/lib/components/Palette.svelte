<script lang="ts">
  import { backend, type TitleHit } from '../api'
  import { app, navigate } from '../stores/app.svelte'
  import { commands } from '../commands/table'
  import { openPage } from '../newPage'

  let q = $state('')
  let pages = $state<TitleHit[]>([])
  let sel = $state(0)
  let input: HTMLInputElement | undefined = $state()
  $effect(() => input?.focus())

  const cmds = $derived(commands.filter((c) => c.run && c.title.toLowerCase().includes(q.toLowerCase())))
  type Item = { label: string; detail: string; run: () => void }
  const items = $derived<Item[]>([
    ...pages.map((p) => ({
      label: p.name,
      detail: 'page',
      run: () => navigate({ kind: 'page', name: p.name, zoom: [] }),
    })),
    ...cmds.map((c) => ({ label: c.title, detail: c.keys, run: () => c.run?.() })),
    // a name that is no page yet can be created from here (Logseq's "Create page")
    ...(q.trim() && !pages.some((p) => p.name.toLowerCase() === q.trim().toLowerCase())
      ? [
          {
            label: `Create page “${q.trim()}”`,
            detail: 'new page',
            run: () => void openPage(q.trim()),
          },
        ]
      : []),
  ])
  async function type() {
    pages = q.trim() ? await backend.call('search_titles', { text: q, limit: 8 }) : []
    sel = 0
  }
  function go(i: Item | undefined) {
    if (!i) return
    app.paletteOpen = false
    i.run()
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') app.paletteOpen = false
    else if (e.key === 'ArrowDown') {
      sel = Math.min(sel + 1, items.length - 1)
      e.preventDefault()
    } else if (e.key === 'ArrowUp') {
      sel = Math.max(sel - 1, 0)
      e.preventDefault()
    } else if (e.key === 'Enter') go(items[sel])
  }
</script>

<div class="overlay" role="presentation" onclick={() => (app.paletteOpen = false)}>
  <div
    class="dialog"
    role="dialog"
    aria-label="Command palette"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={key}
  >
    <input
      bind:this={input}
      bind:value={q}
      oninput={type}
      placeholder="Type a page or command…"
      aria-label="Command or page"
    />
    <ul role="listbox" aria-label="Matches">
      {#each items as it, i (it.label + i)}
        <li role="option" aria-selected={i === sel}>
          <button class:sel={i === sel} onclick={() => go(it)}
            >{it.label}<span class="detail">{it.detail}</span></button
          >
        </li>
      {/each}
    </ul>
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
  input {
    width: 100%;
    padding: 0.6em;
    box-sizing: border-box;
    font-size: 1.05em;
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
