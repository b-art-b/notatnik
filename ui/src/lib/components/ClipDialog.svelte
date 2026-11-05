<script lang="ts">
  // "Clip a web page" (WA-1): an address, _Clip_, progress and _Cancel_. The page becomes a one-chapter book below
  // `assets/web/`; the result goes to whoever opened the dialog (`openClip`), which writes the link.
  // An address that was clipped before asks first: clipping it again replaces the saved copy, the highlights stay.
  import { onDestroy, tick } from 'svelte'
  import { backend, CommandError, type ClippedPage } from '../api'
  import { app, errorMessage } from '../stores/app.svelte'
  import { finishClip, stepLabel, webAddress } from '../commands/clip'

  // `app.clip` is set before the dialog is shown and never null while it is: a snapshot of the page it belongs to
  const page = app.clip?.page ?? ''

  let address = $state('')
  let phase = $state<'form' | 'ask' | 'running' | 'notes'>('form')
  let error = $state('')
  let progress = $state('')
  let result: ClippedPage | null = $state(null)
  let input: HTMLInputElement | undefined = $state()
  // set when the dialog is gone, so a clip that ends afterwards does not write into a closed dialog
  let closed = false
  let timer: ReturnType<typeof setInterval> | undefined

  // the field gets the focus, and a web address on the clipboard (if the browser lets us look) is put in it
  $effect(() => input?.focus())
  void prefill()
  async function prefill() {
    try {
      const found = webAddress(await navigator.clipboard.readText())
      // somebody who started typing is not interrupted
      if (found && !address && !closed) address = found
    } catch {
      // no permission, no clipboard, or not a secure page: the field stays empty
    }
  }

  onDestroy(() => {
    closed = true
    clearInterval(timer)
  })

  function close() {
    if (phase === 'running') void cancel()
    finishClip(null)
  }

  async function cancel() {
    // the clip ends with the error `cancelled`, which `run` turns into a quiet return to the form
    try {
      await backend.call('clip_cancel', {})
    } catch {
      // nothing runs any more
    }
  }

  function watch() {
    timer = setInterval(async () => {
      try {
        const p = await backend.call('clip_progress', {})
        if (p.running && !closed) progress = stepLabel(p.step, p.done, p.total)
      } catch {
        // the next poll tries again
      }
    }, 300)
  }

  async function start(again: boolean) {
    error = ''
    const url = webAddress(address)
    if (!url) {
      error = 'Type or paste a web address that starts with http:// or https://'
      return
    }
    if (phase === 'running') return
    if (!again) {
      try {
        if (await backend.call('clip_existing', { url })) {
          phase = 'ask'
          return
        }
      } catch {
        // not knowing is not a reason to stop: the clip itself answers
      }
    }
    phase = 'running'
    progress = stepLabel('fetching', 0, 0)
    watch()
    try {
      const clip = await backend.call('clip_page', { url, page })
      if (closed) return
      if (clip.notes.length > 0) {
        result = clip
        phase = 'notes'
      } else finishClip(clip)
    } catch (e) {
      if (closed) return
      phase = 'form'
      // the button that was pressed is gone: the focus goes back to the field, so the keyboard still reaches the dialog
      await tick()
      input?.focus()
      // cancelling is what the person asked for, not an error
      if (!(e instanceof CommandError && e.code === 'cancelled')) error = errorMessage(e)
    } finally {
      clearInterval(timer)
    }
  }

  // a changed address is another page: the question about the old one no longer applies
  function edited() {
    error = ''
    if (phase === 'ask') phase = 'form'
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      close()
    } else if (
      e.key === 'Enter' &&
      (phase === 'form' || phase === 'ask') &&
      (e.target as HTMLElement).tagName === 'INPUT'
    ) {
      e.preventDefault()
      void start(phase === 'ask')
    }
  }
</script>

<div class="overlay" role="presentation" onclick={() => phase !== 'running' && close()}>
  <div
    class="dialog"
    role="dialog"
    aria-label="Clip a web page"
    aria-modal="true"
    tabindex="-1"
    data-testid="clip-dialog"
    onclick={(e) => e.stopPropagation()}
    onkeydown={key}
  >
    <h2>Clip a web page</h2>
    {#if phase === 'notes' && result}
      <p>Saved “{result.title}”. Things to know:</p>
      <ul data-testid="clip-notes">
        {#each result.notes as note (note)}<li>{note}</li>{/each}
      </ul>
      <div class="row">
        <button type="button" onclick={() => finishClip(result)} data-testid="clip-insert"
          >Insert the link</button
        >
      </div>
    {:else}
      <p class="muted">
        The article is saved as a book in the graph, so you can read it offline, highlight and comment on it.
      </p>
      <label>
        Address
        <input
          bind:this={input}
          bind:value={address}
          oninput={edited}
          disabled={phase === 'running'}
          placeholder="https://example.org/article"
          inputmode="url"
          autocomplete="off"
          spellcheck="false"
          data-testid="clip-address"
        />
      </label>
      {#if phase === 'ask'}
        <p class="warn" data-testid="clip-again">
          This page was clipped before. Clipping it again replaces the saved copy; your highlights stay.
        </p>
      {/if}
      {#if phase === 'running'}
        <p role="status" data-testid="clip-progress">{progress}</p>
      {/if}
      {#if error}<p class="error" role="alert" data-testid="clip-error">{error}</p>{/if}
      <div class="row">
        {#if phase === 'running'}
          <button type="button" onclick={cancel} data-testid="clip-cancel">Cancel</button>
        {:else}
          <button
            type="button"
            disabled={!address.trim()}
            onclick={() => start(phase === 'ask')}
            data-testid="clip-go">{phase === 'ask' ? 'Clip it again' : 'Clip'}</button
          >
          <button type="button" class="plain" onclick={close}>Close</button>
        {/if}
      </div>
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
    z-index: 60;
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
  @media (pointer: coarse) {
    button,
    input {
      min-height: 44px;
    }
  }
</style>
