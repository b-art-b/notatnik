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
  type Item = { label: string; detail: string; run: () => void; create?: boolean }
  // a name that matches no page can be created right here (Logseq's "Create page"); it is the last row, so a page or command that matches wins Enter (Ctrl+Enter always creates)
  const creatable = $derived(
    !!q.trim() && !pages.some((p) => p.name.toLowerCase() === q.trim().toLowerCase()),
  )
  const items = $derived<Item[]>([
    ...pages.map((p) => ({
      label: p.name,
      detail: 'page',
      run: () => navigate({ kind: 'page', name: p.name, zoom: [] }),
    })),
    ...cmds.map((c) => ({ label: c.title, detail: c.keys, run: () => c.run?.() })),
    ...(creatable
      ? [{ label: `Create page “${q.trim()}”`, detail: 'Ctrl+Enter', run: () => {}, create: true }]
      : []),
  ])
  async function type() {
    problem = ''
    pages = q.trim() ? await backend.call('search_titles', { text: q, limit: 8 }) : []
    sel = 0
  }
  let problem = $state('')
  /** Makes the page named as typed (a file only once something is written); the server's reason if it refuses. */
  async function create() {
    const err = await openPage(q)
    if (err) problem = err
    else app.paletteOpen = false
  }
  function go(i: Item | undefined) {
    if (!i) return
    if (i.create) return void create()
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
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      // Ctrl+Enter: a page named as typed, whatever else matches
      e.preventDefault()
      if (q.trim()) void create()
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
      placeholder="Type a page or command… (Ctrl+Enter creates a page)"
      aria-label="Command or page"
    />
    {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
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
  .problem {
    color: var(--danger);
    margin: 0.4em 0 0;
  }
  .detail {
    color: var(--muted);
    font-size: 0.8em;
  }
</style>
