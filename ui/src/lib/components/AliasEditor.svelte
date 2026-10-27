<script lang="ts">
  // The other names of a page (LN-6): `aliases:` in its front matter. A `[[link]]` with an alias opens the page and counts
  // as one of its backlinks; search and the `[[` list find the page by it. Add one with Enter, remove one with its ×.
  // The backend checks the names (an alias another page has is refused) and writes the front matter.
  import { backend, type PageView } from '../api'
  import { errorMessage } from '../stores/app.svelte'

  let { page, aliases, onchange }: { page: string; aliases: string[]; onchange: (view: PageView) => void } =
    $props()
  let draft = $state('')
  let error = $state('')
  let busy = $state(false)

  async function set(next: string[]) {
    if (busy) return
    busy = true
    error = ''
    try {
      onchange(await backend.call('page_aliases', { page, aliases: next }))
      draft = ''
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  function add() {
    // commas separate several at once: `a, b`
    const more = draft
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    if (more.length) void set([...aliases, ...more])
  }
</script>

<div class="prop" data-testid="aliases">
  <span class="k" title="Other names of this page: a [[link]] with one of them opens it">aliases</span>
  <div class="v">
    <ul class="chips" aria-label="Aliases">
      {#each aliases as a (a)}
        <li>
          <span data-testid="alias">{a}</span><button
            type="button"
            class="x"
            aria-label={`Remove the alias ${a}`}
            disabled={busy}
            onclick={() => set(aliases.filter((x) => x !== a))}>×</button
          >
        </li>
      {/each}
      <li class="add">
        <input
          bind:value={draft}
          aria-label="Add an alias"
          placeholder={aliases.length ? 'add another' : 'add an alias'}
          data-testid="alias-input"
          disabled={busy}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              add()
            }
          }}
        />
      </li>
    </ul>
    {#if error}<p class="error" role="alert" data-testid="alias-error">{error}</p>{/if}
  </div>
  <span></span>
</div>

<style>
  /* a row of the properties card (a grid of key, value and a spare column), like the other rows */
  .prop {
    display: contents;
  }
  .k {
    color: var(--muted);
    font-weight: 500;
    padding-top: 0.35em;
  }
  .v {
    min-width: 0;
    padding-top: 0.2em;
  }
  .chips {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.3em;
    align-items: center;
  }
  .chips li:not(.add) {
    display: inline-flex;
    align-items: center;
    background: var(--chip);
    border-radius: 999px;
    padding: 0 0.2em 0 0.7em;
  }
  .x {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0 0.4em;
  }
  .x:hover:not(:disabled),
  .x:focus-visible {
    color: var(--danger);
  }
  input {
    border: 1px dashed var(--border);
    border-radius: 999px;
    background: none;
    color: inherit;
    font: inherit;
    padding: 0 0.7em;
    min-width: 8em;
  }
  .error {
    color: var(--danger);
    margin: 0.2em 0 0;
    font-size: 0.9em;
  }
</style>
