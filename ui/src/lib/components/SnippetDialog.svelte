<script lang="ts">
  // "Save as template…": names the selected blocks and stores them as `templates/snippets/<name>.md` (PR-10).
  // `/template` then offers the name and inserts the blocks wherever the cursor is.
  import { backend, CommandError } from '../api'
  import { app, errorMessage, toast } from '../stores/app.svelte'

  // `app.snippetText` is set when the dialog opens and never null while it is shown: `text` is a snapshot of it
  const text = app.snippetText ?? ''
  let name = $state('')
  let error = $state('')
  let exists = $state(false)
  let busy = $state(false)
  let input: HTMLInputElement | undefined = $state()
  $effect(() => input?.focus())

  const blocks = text.split('\n').filter((l) => /^\s*- /.test(l)).length

  function close() {
    app.snippetText = null
  }

  async function save(replace: boolean) {
    if (busy || !name.trim()) return
    busy = true
    error = ''
    try {
      const r = await backend.call('save_snippet', { name, text, replace })
      toast(
        `Saved the template “${r.name}” (${r.blocks} block${r.blocks === 1 ? '' : 's'}). Insert it with /template.`,
      )
      close()
    } catch (e) {
      // a name that is taken: offer to replace it instead of failing
      if (e instanceof CommandError && e.code === 'exists') exists = true
      else error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') close()
    else if (e.key === 'Enter' && !exists) {
      e.preventDefault()
      void save(false)
    }
  }
</script>

<div class="overlay" role="presentation" onclick={close}>
  <div
    class="dialog"
    role="dialog"
    aria-label="Save as template"
    aria-modal="true"
    tabindex="-1"
    data-testid="snippet-dialog"
    onclick={(e) => e.stopPropagation()}
    onkeydown={key}
  >
    <h2>Save as template</h2>
    <p class="muted">
      {blocks} block{blocks === 1 ? '' : 's'}, with what is below them. Insert them anywhere with
      <code>/template</code>.
    </p>
    <pre class="preview" aria-label="Blocks to save">{text}</pre>
    <label>
      Name
      <input
        bind:this={input}
        bind:value={name}
        oninput={() => (exists = false)}
        placeholder="Meeting notes"
        data-testid="snippet-name"
      />
    </label>
    {#if exists}
      <p class="warn" data-testid="snippet-exists">
        A template named “{name.trim()}” exists. The old text stays in the file's history.
      </p>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="row">
      {#if exists}
        <button type="button" disabled={busy} onclick={() => save(true)} data-testid="snippet-replace"
          >Replace it</button
        >
      {:else}
        <button
          type="button"
          disabled={busy || !name.trim()}
          onclick={() => save(false)}
          data-testid="snippet-save">Save</button
        >
      {/if}
      <button type="button" class="plain" onclick={close}>Cancel</button>
    </div>
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
    width: min(520px, 92vw);
    max-height: 80vh;
    overflow: auto;
    background: var(--panel);
    border-radius: 10px;
    padding: 1em;
    align-self: flex-start;
  }
  h2 {
    margin: 0 0 0.3em;
    font-size: 1.1em;
  }
  .muted {
    color: var(--muted);
    margin: 0 0 0.6em;
  }
  .preview {
    max-height: 30vh;
    overflow: auto;
    margin: 0 0 0.8em;
    padding: 0.5em;
    border: 1px solid var(--border);
    border-radius: 6px;
    font-size: 0.85em;
    white-space: pre-wrap;
  }
  label {
    display: block;
  }
  input {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 0.5em;
    margin-top: 0.2em;
    font-size: 1em;
  }
  .row {
    display: flex;
    gap: 0.5em;
    margin-top: 0.8em;
  }
  .warn {
    color: var(--warn);
  }
  .error {
    color: var(--danger);
  }
</style>
