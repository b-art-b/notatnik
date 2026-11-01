<script lang="ts">
  // The drawing editor (DR-1 to DR-6): js-draw in a full-screen dialog. js-draw is a plain DOM library, not a Svelte
  // one, so it is created in `onMount` into a `<div>` (`bind:this` gives us the element) and taken apart when the
  // component goes away; what it knows is read when a button is pressed, not tracked as state.
  import { onMount } from 'svelte'
  import Editor, { BackgroundComponentBackgroundType, Color4, PenTool } from 'js-draw'
  import 'js-draw/bundledStyles'
  import { backend } from '../api'
  import { app, toast, errorMessage } from '../stores/app.svelte'
  import { finishDrawing } from '../commands/drawing'

  // `app.drawing` is set when the dialog opens and not null while it is shown: a snapshot is enough
  const target = app.drawing ?? { page: '', path: null }
  const TOOLS_KEY = 'notatnik.drawing.tools'

  let host: HTMLDivElement | undefined = $state()
  let editor: Editor | undefined
  let toolbar: ReturnType<Editor['addToolbar']> | undefined
  // the undo depth right after the drawing was loaded: the drawing is changed when it differs
  let baseline = 0
  let loading = $state(true)
  let saving = $state(false)
  // closing with changes asks first
  let confirming = $state(false)

  const isEmpty = () => (editor?.image.getAllComponents().length ?? 0) === 0
  const changed = () => !!editor && editor.history.undoStackSize !== baseline

  /** A pen writes what the hand wrote (DR-3): no shape correction, no pull toward a smoother path, and the
   * pressure of a stylus changes the width. */
  function keepHandwriting(ed: Editor) {
    for (const pen of ed.toolController.getMatchingTools(PenTool)) {
      pen.setStrokeAutocorrectEnabled(false)
      pen.setHasStabilization(false)
      pen.setPressureSensitivityEnabled(true)
    }
  }

  function restoreTools() {
    try {
      const saved = localStorage.getItem(TOOLS_KEY)
      if (saved) toolbar?.deserializeState(saved)
    } catch {
      /* no storage, or a state of another version: the pens start as they are */
    }
  }

  function rememberTools() {
    try {
      if (toolbar) localStorage.setItem(TOOLS_KEY, toolbar.serializeState())
    } catch {
      /* the choice is just not remembered */
    }
  }

  onMount(() => {
    if (!host) return
    const ed = new Editor(host, { wheelEventsEnabled: true })
    editor = ed
    ed.getRootElement().style.height = '100%'
    toolbar = ed.addToolbar()
    // ink is black until the person chose another colour, which is remembered with the other tool settings
    for (const pen of ed.toolController.getMatchingTools(PenTool)) pen.setColor(Color4.black)
    restoreTools()
    keepHandwriting(ed)
    void (async () => {
      try {
        if (target.path) {
          const text = (await backend.call('read_drawing', { path: target.path })).value
          await ed.loadFromSVG(text, true)
          ed.dispatch(ed.image.setAutoresizeEnabled(true), false)
        } else {
          // a white sheet: the file reads on a dark theme as well, and the canvas grows as it is drawn
          ed.dispatch(
            ed.setBackgroundStyle({
              color: Color4.white,
              type: BackgroundComponentBackgroundType.SolidColor,
              autoresize: true,
            }),
            false,
          )
        }
        baseline = ed.history.undoStackSize
      } catch (e) {
        toast(errorMessage(e), 'error')
        finishDrawing(null)
        return
      }
      loading = false
      ed.focus()
    })()
    return () => {
      toolbar?.remove()
      ed.remove()
      editor = undefined
    }
  })

  /** Done: a new drawing that is empty, or an old one that did not change, stores nothing. */
  async function done() {
    const ed = editor
    if (!ed || saving || loading) return
    if (!changed() || (!target.path && isEmpty())) {
      rememberTools()
      finishDrawing(null)
      return
    }
    saving = true
    try {
      const svg = new XMLSerializer().serializeToString(ed.toSVG())
      const saved = await backend.call('save_drawing', { page: target.page, path: target.path, svg })
      rememberTools()
      finishDrawing(saved)
    } catch (e) {
      toast(errorMessage(e), 'error')
      saving = false
    }
  }

  /** Cancel, and Escape: closes at once when nothing changed, else asks. */
  function cancel() {
    if (saving) return
    if (changed()) {
      confirming = true
      return
    }
    rememberTools()
    finishDrawing(null)
  }

  function key(e: KeyboardEvent) {
    // the drawing keys (undo, delete, ...) are js-draw's; the page behind must not see any of them
    e.stopPropagation()
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault()
      if (confirming) {
        confirming = false
        editor?.focus()
      } else cancel()
    }
  }
</script>

<div class="overlay" role="presentation">
  <!-- the dialog is a div: js-draw puts its own toolbar and canvas inside -->
  <div
    class="dialog"
    role="dialog"
    aria-label="Drawing"
    aria-modal="true"
    tabindex="-1"
    data-testid="drawing-editor"
    onkeydown={key}
  >
    <header>
      <h2>Drawing</h2>
      <span class="hint">Pen draws; with a stylus a finger moves and zooms the canvas</span>
      <div class="actions">
        <button type="button" class="plain" onclick={cancel} data-testid="drawing-cancel">Cancel</button>
        <button type="button" onclick={done} disabled={loading || saving} data-testid="drawing-done"
          >{saving ? 'Saving…' : 'Done'}</button
        >
      </div>
    </header>
    {#if confirming}
      <div class="confirm" role="alertdialog" aria-label="Discard the drawing?" data-testid="drawing-confirm">
        <span>Discard what you drew?</span>
        <button
          type="button"
          data-testid="drawing-keep"
          onclick={() => {
            confirming = false
            // the button that had the focus is gone: give it back to the canvas, or Escape would not reach us
            editor?.focus()
          }}>Keep drawing</button
        >
        <button
          type="button"
          class="danger"
          data-testid="drawing-discard"
          onclick={() => {
            rememberTools()
            finishDrawing(null)
          }}>Discard</button
        >
      </div>
    {/if}
    <div class="host" bind:this={host} class:loading></div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    z-index: 60;
    display: flex;
  }
  .dialog {
    display: flex;
    flex-direction: column;
    flex: 1;
    margin: 2vh 2vw;
    min-height: 0;
    background: var(--panel);
    border-radius: 10px;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.8em;
    padding: 0.5em 0.9em;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 1.05em;
  }
  .hint {
    color: var(--muted);
    font-size: 0.85em;
    flex: 1;
  }
  .actions {
    display: flex;
    gap: 0.5em;
  }
  .actions button,
  .confirm button {
    font: inherit;
    padding: 0.35em 1.1em;
    border-radius: 8px;
    cursor: pointer;
    border: 1px solid var(--border);
    background: transparent;
    color: inherit;
  }
  .actions button:not(.plain) {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .actions button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .confirm {
    display: flex;
    align-items: center;
    gap: 0.6em;
    padding: 0.5em 0.9em;
    background: var(--chip);
    border-bottom: 1px solid var(--border);
  }
  .confirm span {
    flex: 1;
  }
  .confirm .danger {
    color: var(--danger);
    border-color: var(--danger);
  }
  .host {
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .host.loading {
    opacity: 0.5;
  }
</style>
