<script lang="ts">
  // The text of one template (`templates/<name>.md`) in a text box: the properties between the `---` lines are the
  // fields a page of that type should have, the blocks below them are its starting content. The file is shown and
  // saved as it is, so nothing is hidden or reformatted. The text a save replaces stays in the file's history.
  import { backend, type TemplateEntry } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'

  let {
    name,
    onsaved,
    onclose,
  }: { name: string; onsaved: (entry: TemplateEntry) => void; onclose: () => void } = $props()

  let original = $state<string | null>(null)
  let text = $state('')
  let busy = $state(false)
  let error = $state('')
  const dirty = $derived(original !== null && text !== original)

  $effect(() => {
    const wanted = name
    backend
      .call('read_template', { name: wanted })
      .then((r) => {
        original = r.value
        text = r.value
      })
      .catch((e) => (error = errorMessage(e)))
  })

  async function save() {
    busy = true
    error = ''
    try {
      const entry = await backend.call('write_template', { name, text })
      original = text.endsWith('\n') || !text ? text : text + '\n'
      text = original
      onsaved(entry)
      toast(`Saved the template “${name}”`)
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  function key(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && dirty && !busy) {
      e.preventDefault()
      void save()
    } else if (e.key === 'Escape') {
      e.stopPropagation()
      onclose()
    }
  }
</script>

<div class="editor" data-testid="template-editor">
  {#if original === null && !error}
    <p class="muted">Loading…</p>
  {:else}
    <textarea
      aria-label={`Text of the template ${name}`}
      bind:value={text}
      onkeydown={key}
      spellcheck="false"
      rows={Math.min(24, Math.max(8, text.split('\n').length + 1))}></textarea>
    <p class="hint">
      Properties go between two <code>---</code> lines at the top, blocks start with <code>- </code> (indent a
      child block with two spaces). <code>&#123;&#123;title&#125;&#125;</code>,
      <code>&#123;&#123;date&#125;&#125;</code> and the other placeholders are filled in when the template is used.
    </p>
    <span class="actions">
      <button type="button" disabled={!dirty || busy} onclick={save} data-testid="template-save">Save</button>
      <button type="button" class="plain" onclick={onclose}>{dirty ? 'Discard' : 'Close'}</button>
      {#if dirty}<span class="muted">Unsaved changes (Ctrl+Enter saves)</span>{/if}
    </span>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .editor {
    display: grid;
    gap: 0.5em;
    flex-basis: 100%;
    margin: 0.3em 0 0.6em;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    font-family: var(--font-mono);
    font-size: 0.9em;
    line-height: 1.5;
    padding: 0.6em 0.8em;
    color: inherit;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    resize: vertical;
    tab-size: 2;
  }
  textarea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .hint,
  .muted {
    margin: 0;
    color: var(--muted);
    font-size: 0.85em;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 0.8em;
    flex-wrap: wrap;
  }
  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
