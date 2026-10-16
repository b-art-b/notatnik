<script lang="ts">
  import { backend, type SearchHitView } from '../api'
  import { app, errorMessage } from '../stores/app.svelte'
  import { jumpToBlock } from '../jump'

  let q = $state('')
  let hits = $state<SearchHitView[]>([])
  let sel = $state(0)
  let problem = $state('') // a mistake in the search syntax, in words
  let input: HTMLInputElement | undefined = $state()
  let timer: ReturnType<typeof setTimeout> | undefined

  $effect(() => {
    input?.focus()
  })
  function run() {
    clearTimeout(timer)
    timer = setTimeout(async () => {
      try {
        hits = q.trim() ? await backend.call('search', { text: q, limit: 30 }) : []
        problem = ''
      } catch (e) {
        // half-typed syntax (`(budget`) is normal while typing: say what is wrong, keep the last results
        problem = errorMessage(e)
      }
      sel = 0
    }, 30)
  }
  function open(h: SearchHitView) {
    app.searchOpen = false
    jumpToBlock(h.page, h.path)
  }
  function marked(h: SearchHitView): { t: string; hl: boolean }[] {
    const out: { t: string; hl: boolean }[] = []
    let pos = 0
    for (const [s, e] of h.highlights) {
      out.push({ t: h.text.slice(pos, s), hl: false }, { t: h.text.slice(s, e), hl: true })
      pos = e
    }
    out.push({ t: h.text.slice(pos), hl: false })
    return out
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') app.searchOpen = false
    else if (e.key === 'ArrowDown') {
      sel = Math.min(sel + 1, hits.length - 1)
      e.preventDefault()
    } else if (e.key === 'ArrowUp') {
      sel = Math.max(sel - 1, 0)
      e.preventDefault()
    } else if (e.key === 'Enter' && hits[sel]) open(hits[sel])
  }
</script>

<div class="overlay" role="presentation" onclick={() => (app.searchOpen = false)}>
  <div
    class="dialog"
    role="dialog"
    aria-label="Search"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={key}
  >
    <input
      bind:this={input}
      bind:value={q}
      oninput={run}
      placeholder="Search pages and blocks…"
      aria-label="Search text"
    />
    {#if problem}
      <p class="problem" role="alert">{problem}</p>
    {:else}
      <p class="syntax" data-testid="search-syntax">
        <code>a b</code> both · <code>a OR b</code> either · <code>-a</code> without · <code>"a b"</code>
        phrase ·
        <code>a*</code> prefix · <code>( )</code> group
      </p>
    {/if}
    <ul role="listbox" aria-label="Results">
      {#each hits as h, i (i)}
        <li role="option" aria-selected={i === sel} class:sel={i === sel}>
          <button onclick={() => open(h)} data-testid="search-hit">
            <span class="kind">{h.kind}</span><span class="page">{h.page}</span>
            <span class="text"
              >{#each marked(h) as m, j (j)}{#if m.hl}<mark>{m.t}</mark>{:else}{m.t}{/if}{/each}</span
            >
          </button>
        </li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .syntax,
  .problem {
    margin: 0.4em 0 0;
    font-size: 0.8em;
  }
  .syntax {
    color: var(--muted);
  }
  .syntax code {
    background: var(--chip);
    padding: 0 0.3em;
    border-radius: 3px;
  }
  .problem {
    color: var(--danger);
  }
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
    width: min(720px, 92vw);
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
    font-size: 1.05em;
    box-sizing: border-box;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0.5em 0 0;
  }
  li button {
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    padding: 0.4em;
    cursor: pointer;
    color: var(--text);
    display: flex;
    gap: 0.6em;
  }
  li.sel button {
    background: var(--selection);
  }
  .kind {
    color: var(--muted);
    font-size: 0.75em;
    width: 3em;
  }
  .page {
    color: var(--muted);
    font-size: 0.85em;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
