<script lang="ts">
  // A query pinned to the sidebar: the query on its own page, with Edit (name and text, kept where it stands in the
  // list) and Remove. The page belongs to the active graph, like every page of the page area.
  import { backend, type SavedQueryView } from '../api'
  import { app, errorMessage, navigate, toast } from '../stores/app.svelte'
  import QueryBlock from './QueryBlock.svelte'

  let { name }: { name: string } = $props()
  let q = $state<SavedQueryView | null | undefined>(undefined)
  $effect(() => {
    void app.savedVersion
    backend.call('list_saved_queries', {}).then((l) => (q = l.find((x) => x.name === name) ?? null))
  })

  // the edit form: opened with what the query says now
  let editing = $state(false)
  let newName = $state('')
  let text = $state('')
  let error = $state('')
  let busy = $state(false)

  function edit() {
    if (!q) return
    newName = q.name
    text = q.query
    error = ''
    editing = true
  }

  async function save() {
    if (busy) return
    busy = true
    error = ''
    try {
      await backend.call('edit_saved_query', { old: name, name: newName.trim(), query: text })
      app.savedVersion++
      editing = false
      toast(`Saved “${newName.trim()}”`)
      // a new name is a new address
      if (newName.trim() !== name) navigate({ kind: 'saved', name: newName.trim() })
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  async function remove() {
    await backend.call('delete_saved_query', { name })
    app.savedVersion++
    navigate({ kind: 'journals' })
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation()
      editing = false
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      void save()
    }
  }
</script>

<h1>{name}</h1>
{#if q}
  {#if editing}
    <!-- the keys (Esc cancels, Ctrl+Enter saves) are handled where the fields are, on the form -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <form
      class="edit"
      aria-label="Edit saved query"
      data-testid="saved-edit"
      onsubmit={(e) => {
        e.preventDefault()
        void save()
      }}
      onkeydown={key}
    >
      <label>
        Name in the sidebar
        <input bind:value={newName} data-testid="saved-name" />
      </label>
      <label>
        Query (one <code>key: value</code> clause per line)
        <textarea bind:value={text} rows="6" spellcheck="false" data-testid="saved-text"></textarea>
      </label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="row">
        <button type="submit" disabled={busy || !newName.trim()} data-testid="saved-save">Save</button>
        <button type="button" class="plain" onclick={() => (editing = false)}>Cancel</button>
        <span class="hint">Ctrl+Enter saves, Esc cancels</span>
      </div>
    </form>
  {:else}
    <div class="actions">
      <button class="link" data-testid="saved-edit-open" onclick={edit}>Edit</button>
      <button class="link" onclick={remove}>Remove from sidebar</button>
    </div>
  {/if}
  {#key q.query}<QueryBlock text={q.query} saved />{/key}
{:else if q === null}
  <p class="muted">No saved query called “{name}”.</p>
{/if}

<style>
  .actions {
    display: flex;
    gap: 1em;
    margin: 0 0 0.6em;
  }
  .edit {
    border: 1px solid var(--border);
    background: var(--panel);
    border-radius: 8px;
    padding: 0.8em;
    margin: 0 0 0.8em;
  }
  label {
    display: block;
    margin-bottom: 0.6em;
  }
  input,
  textarea {
    display: block;
    width: 100%;
    box-sizing: border-box;
    margin-top: 0.2em;
    padding: 0.4em;
    font: inherit;
  }
  textarea {
    font-family: var(--font-mono);
    font-size: 0.9em;
    resize: vertical;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5em;
  }
  .hint {
    color: var(--muted);
    font-size: 0.8em;
    margin-left: auto;
  }
  .error {
    color: var(--danger);
    margin: 0 0 0.6em;
  }
</style>
