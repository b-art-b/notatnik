<script lang="ts">
  // AN-5: the capture screen. One input; "Add" puts the text at the end of today's journal and goes back. The Android
  // widget (AN-2, AN-6) opens it through `notatnik://capture`.
  import { onMount } from 'svelte'
  import { backend } from '../api'
  import { app, errorMessage, navigate, toast } from '../stores/app.svelte'

  let text = $state('')
  let input: HTMLTextAreaElement | undefined = $state()
  let problem = $state('')
  let busy = $state(false)
  // a plain variable, not state: it is only read when the screen closes
  let cameFrom = app.route

  onMount(() => input?.focus())

  function leave() {
    // back to where we were; when the app was opened on this screen there is no such place: the journal
    if (cameFrom.kind !== 'capture' && history.length > 1) history.back()
    else navigate({ kind: 'journals' })
  }

  async function add() {
    if (busy) return
    if (!text.trim()) {
      problem = 'There is nothing to add.'
      input?.focus()
      return
    }
    busy = true
    try {
      const r = await backend.call('capture', { text, page: null })
      toast(`Added to ${r.value}.`)
      text = ''
      leave()
    } catch (e) {
      problem = errorMessage(e)
    } finally {
      busy = false
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      void add()
    } else if (e.key === 'Escape') {
      leave()
    }
  }
</script>

<section class="capture" aria-label="Capture">
  <h1>Capture</h1>
  <label for="capture-text">Add to today's journal</label>
  <textarea
    id="capture-text"
    bind:this={input}
    bind:value={text}
    {onkeydown}
    oninput={() => (problem = '')}
    rows="6"
    placeholder="Type a note…"
    enterkeyhint="done"></textarea>
  {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
  <div class="row">
    <button type="button" class="primary" onclick={add} disabled={busy}>Add</button>
    <button type="button" onclick={leave}>Cancel</button>
  </div>
</section>

<style>
  .capture {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    max-width: 40rem;
  }
  h1 {
    font-size: 1.3rem;
    margin: 0;
  }
  textarea {
    font: inherit;
    width: 100%;
    box-sizing: border-box;
    padding: 0.6rem;
    border: 1px solid var(--border, #8886);
    border-radius: 6px;
    background: var(--bg, transparent);
    color: inherit;
    resize: vertical;
    min-height: 8rem;
  }
  .row {
    display: flex;
    gap: 0.6rem;
  }
  button {
    min-height: 44px;
    padding: 0 1.2rem;
    font: inherit;
  }
  .problem {
    color: var(--danger, #c0392b);
    margin: 0;
  }
</style>
