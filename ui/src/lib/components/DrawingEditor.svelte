<script lang="ts">
  // The drawing editor (DR-1 to DR-6): js-draw in a full-screen dialog. js-draw is a plain DOM library, not a Svelte
  // one, so it is created in `onMount` into a `<div>` (`bind:this` gives us the element) and taken apart when the
  // component goes away; what it knows is read when a button is pressed, not tracked as state.
  import { onMount } from 'svelte'
  import Editor, {
    BackgroundComponentBackgroundType,
    Color4,
    PanZoomMode,
    PanZoomTool,
    PenTool,
    makeFreehandLineBuilder,
    makePressureSensitiveFreehandLineBuilder,
  } from 'js-draw'
  import 'js-draw/bundledStyles'
  import { backend } from '../api'
  import { app, toast, errorMessage } from '../stores/app.svelte'
  import { drawingMarkdown, finishDrawing, type DrawingMode, type DrawingOpen } from '../commands/drawing'
  import { makeRawInkBuilder } from '../rawInk'
  import { MakeSpaceTool } from '../makeSpaceTool'

  // `app.drawing` is set when the dialog opens and not null while it is shown: a snapshot is enough
  const target: DrawingOpen = app.drawing ?? {
    page: null,
    path: null,
    name: '',
    mode: 'picture',
    host: null,
  }
  const isNew = !target.path
  const TOOLS_KEY = 'notatnik.drawing.tools'
  // set once a stylus was seen on this device: from then on one finger moves the canvas, so a resting hand does not draw
  const STYLUS_KEY = 'notatnik.drawing.stylus'

  let host: HTMLDivElement | undefined = $state()
  let editor: Editor | undefined
  let toolbar: ReturnType<Editor['addToolbar']> | undefined
  // "Make space" (DR-12): whether the tool is on, and where its line is while dragging (client pixels, null: none).
  // Both are `$state` because the template shows them; the tool itself is a plain js-draw object.
  let spaceOn = $state(false)
  let spaceLine = $state<number | null>(null)
  // the undo depth right after the drawing was loaded: the drawing is changed when it differs
  let baseline = 0
  let loading = $state(true)
  let saving = $state(false)
  // closing with changes asks first; deleting always asks
  let confirming = $state(false)
  let deleting = $state(false)
  // how the drawing is written in the note: its name (the label of the link) and picture or link. `$state` makes
  // the inputs follow them; the page the drawing is in is only rewritten when Done is pressed
  let name = $state(target.name)
  let mode = $state<DrawingMode>(target.mode)
  // an existing drawing can only be renamed or switched where a block holds it (a table cell cannot rewrite one)
  const canDescribe = isNew || !!target.host

  const isEmpty = () => (editor?.image.getAllComponents().length ?? 0) === 0
  const strokesChanged = () => !!editor && editor.history.undoStackSize !== baseline
  const metaChanged = () => name.trim() !== target.name.trim() || mode !== target.mode
  /** Something would be lost by closing: strokes, or for an existing drawing a new name or form. */
  const dirty = () => (isNew ? !isEmpty() : strokesChanged() || metaChanged())

  /** A pen writes what the hand wrote (DR-3): no shape correction, no pull toward a smoother path, no curve fitted
   * to the points (`rawInk.ts` keeps all of them), and the pressure of a stylus changes the width. */
  function keepHandwriting(ed: Editor) {
    for (const pen of ed.toolController.getMatchingTools(PenTool)) {
      // the stock freehand pens fit a curve; a shape pen the person chose (line, arrow, ...) stays as it is
      const f = pen.getStrokeFactory()
      if (f === makeFreehandLineBuilder || f === makePressureSensitiveFreehandLineBuilder) {
        pen.setStrokeFactory(makeRawInkBuilder)
      }
      pen.setStrokeAutocorrectEnabled(false)
      pen.setHasStabilization(false)
      pen.setPressureSensitivityEnabled(true)
    }
  }

  /** One finger moves the canvas (two fingers zoom) instead of drawing; the pen and the mouse still draw (DR-2). */
  function fingerMovesCanvas(ed: Editor, on: boolean) {
    for (const t of ed.toolController.getMatchingTools(PanZoomTool)) {
      t.setModeEnabled(PanZoomMode.OneFingerTouchGestures, on)
    }
  }

  /** The first time a stylus touches the canvas, fingers stop drawing, now and in the next sessions. */
  function noticeStylus(e: PointerEvent) {
    if (e.pointerType !== 'pen' || !editor) return
    fingerMovesCanvas(editor, true)
    try {
      localStorage.setItem(STYLUS_KEY, '1')
    } catch {
      /* it is just noticed again next time */
    }
  }

  /** The toolbar icon: a line with an arrow down and an arrow up, the gap that opens. */
  function makeSpaceIcon(): SVGSVGElement {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
    svg.setAttribute('viewBox', '0 0 24 24')
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path')
    path.setAttribute('d', 'M3 12h18M12 9V3m-3 3 3-3 3 3M12 15v6m-3-3 3 3 3-3')
    path.setAttribute('fill', 'none')
    path.setAttribute('stroke', 'currentColor')
    path.setAttribute('stroke-width', '2')
    path.setAttribute('stroke-linecap', 'round')
    path.setAttribute('stroke-linejoin', 'round')
    svg.appendChild(path)
    return svg
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
    const ed = new Editor(host, {
      wheelEventsEnabled: true,
      // the pen list offers the handwriting pen and the shape pens, not the two that fit a curve to the stroke
      pens: {
        additionalPenTypes: [{ name: 'Handwriting', id: 'handwriting', factory: makeRawInkBuilder }],
        filterPenTypes: (p) => p.id !== 'freehand-pen' && p.id !== 'pressure-sensitive-pen',
      },
    })
    editor = ed
    ed.getRootElement().style.height = '100%'
    // a primary tool: choosing a pen in the toolbar switches it off. It must be added before the toolbar is made.
    const space = new MakeSpaceTool(ed, (y) => (spaceLine = y))
    ed.toolController.addPrimaryTool(space)
    toolbar = ed.addToolbar()
    const spaceButton = toolbar.addActionButton({ label: 'Make space', icon: makeSpaceIcon() }, () =>
      space.setEnabled(true),
    )
    // the button looks pressed while the tool is on; `onUpdate` runs now and at every change
    space.enabledValue().onUpdate((on) => {
      spaceOn = on
      spaceButton.setSelected(on)
    })
    // ink is black until the person chose another colour, which is remembered with the other tool settings
    for (const pen of ed.toolController.getMatchingTools(PenTool)) pen.setColor(Color4.black)
    restoreTools()
    keepHandwriting(ed)
    try {
      if (localStorage.getItem(STYLUS_KEY)) fingerMovesCanvas(ed, true)
    } catch {
      /* no storage: fingers draw until a stylus is seen */
    }
    // capture: before js-draw handles the same event
    host.addEventListener('pointerdown', noticeStylus, true)
    void (async () => {
      try {
        if (target.path) {
          const text = (await backend.call('read_drawing', { path: target.path })).value
          await ed.loadFromSVG(text, true)
          ed.dispatch(ed.image.setAutoresizeEnabled(true), false)
        } else {
          // a white sheet with a grid (an existing drawing keeps the background it was saved with): the file reads on a
          // dark theme as well, and the canvas grows as it is drawn. Not in the undo history.
          ed.dispatch(
            ed.setBackgroundStyle({
              color: Color4.white,
              type: BackgroundComponentBackgroundType.Grid,
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
      host?.removeEventListener('pointerdown', noticeStylus, true)
      toolbar?.remove()
      ed.remove()
      editor = undefined
    }
  })

  /** Done: a new drawing that is empty, or an old one with nothing changed, stores nothing. The file is written when
   * the strokes changed; a new name or form only rewrites the reference in the block. */
  async function done() {
    const ed = editor
    if (!ed || saving || loading) return
    if (isNew ? isEmpty() : !strokesChanged() && !metaChanged()) {
      rememberTools()
      finishDrawing(null)
      return
    }
    saving = true
    try {
      let path = target.path
      let markdown = ''
      if (isNew || strokesChanged()) {
        const svg = new XMLSerializer().serializeToString(ed.toSVG())
        const saved = await backend.call('save_drawing', { page: target.page, path: target.path, svg })
        path = saved.path
        // `![](…)` as the server wrote it: the path as this page links it, to write again with a name
        if (isNew) {
          const href = /^!\[\]\((.*)\)$/.exec(saved.markdown)?.[1] ?? saved.path
          markdown = drawingMarkdown(href, name, mode)
        }
      }
      // an existing drawing: the block writes the new name and form around the target it already has
      if (!isNew && target.host && path && metaChanged())
        await target.host.replace(path, (href) => drawingMarkdown(href, name, mode))
      rememberTools()
      finishDrawing({ path: path ?? '', markdown })
    } catch (e) {
      toast(errorMessage(e), 'error')
      saving = false
    }
  }

  /** Delete (after asking): the reference goes out of the block, then the file to the trash if no other page
   * links it. */
  async function remove() {
    const path = target.path
    if (!path || !target.host || saving) return
    saving = true
    try {
      await target.host.replace(path, null)
      const unused = (await backend.call('list_orphan_assets', {})).items.some((i) => i.path === path)
      if (unused) await backend.call('delete_orphan_assets', { paths: [path] })
      else toast('Another page links this drawing too, so its file stays.')
      rememberTools()
      finishDrawing({ path, markdown: '', deleted: true })
    } catch (e) {
      toast(errorMessage(e), 'error')
      saving = false
    }
  }

  /** Cancel, and Escape: closes at once when nothing changed, else asks. */
  function cancel() {
    if (saving) return
    if (dirty()) {
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
      if (confirming || deleting) {
        confirming = false
        deleting = false
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
      <label class="name">
        <span class="sr">Name</span>
        <input
          bind:value={name}
          placeholder="Name (optional)"
          maxlength="120"
          disabled={!canDescribe}
          title={canDescribe ? '' : 'The name is changed where the drawing is written in a block'}
          data-testid="drawing-name"
          aria-label="Name"
        />
      </label>
      <fieldset class="mode" disabled={!canDescribe}>
        <legend class="sr">Show in the note as</legend>
        <label title="The drawing is shown in the note" class:on={mode === 'picture'}
          ><input type="radio" name="drawing-mode" value="picture" bind:group={mode} /> Picture</label
        >
        <label title="Only a link is written; a click opens the drawing" class:on={mode === 'link'}
          ><input type="radio" name="drawing-mode" value="link" bind:group={mode} /> Link</label
        >
      </fieldset>
      <span class="spacer"></span>
      <div class="actions">
        {#if !isNew && target.host}
          <button
            type="button"
            class="plain danger"
            onclick={() => (deleting = true)}
            disabled={saving}
            data-testid="drawing-delete">Delete…</button
          >
        {/if}
        <button type="button" class="plain" onclick={cancel} data-testid="drawing-cancel">Cancel</button>
        <button type="button" onclick={done} disabled={loading || saving} data-testid="drawing-done"
          >{saving ? 'Saving…' : 'Done'}</button
        >
      </div>
    </header>
    {#if deleting}
      <div
        class="confirm"
        role="alertdialog"
        aria-label="Delete the drawing?"
        data-testid="drawing-delete-confirm"
      >
        <span>Delete this drawing? It is removed from the note and its file goes to the trash.</span>
        <button
          type="button"
          data-testid="drawing-delete-keep"
          onclick={() => {
            deleting = false
            editor?.focus()
          }}>Keep it</button
        >
        <button type="button" class="danger" data-testid="drawing-delete-yes" onclick={remove}>Delete</button>
      </div>
    {/if}
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
    <!-- the help sits under the canvas, out of the way of the controls above -->
    <p class="hint" data-testid="drawing-hint">
      {#if spaceOn}Make space: press where the space should start and drag; everything below follows. Pick a pen
        to stop{:else}The pen draws; once a stylus has been used a finger moves the canvas and two fingers
        zoom{/if}
    </p>
    {#if spaceLine !== null}
      <!-- the line of "Make space": fixed on the page at the height the tool reports -->
      <div class="space-line" style:top="{spaceLine}px" data-testid="space-line" aria-hidden="true"></div>
    {/if}
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
  .space-line {
    position: fixed;
    left: 0;
    right: 0;
    height: 0;
    border-top: 2px dashed var(--accent, #2563eb);
    pointer-events: none;
    z-index: 70;
  }
  /* the help line under the canvas */
  .hint {
    margin: 0;
    padding: 0.4em 0.9em;
    color: var(--muted);
    font-size: 0.8em;
    border-top: 1px solid var(--border);
  }
  .spacer {
    flex: 1;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .name input {
    font: inherit;
    padding: 0.3em 0.6em;
    width: 15em;
    max-width: 40vw;
    color: inherit;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  /* Picture | Link as one segmented control; the radio itself is hidden, the label is the target */
  .mode {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    margin: 0;
    padding: 2px;
    gap: 2px;
  }
  .mode label {
    position: relative;
    padding: 0.25em 0.9em;
    border-radius: 6px;
    cursor: pointer;
  }
  .mode label.on {
    background: var(--chip);
    font-weight: 600;
  }
  .mode label:has(input:focus-visible) {
    outline: 2px solid var(--accent);
  }
  .mode input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .mode:disabled {
    opacity: 0.6;
  }
  .actions .danger {
    color: var(--danger);
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
  /* js-draw's toolbar is built by the library, so its rules are reached through :global. Its defaults (60 px buttons
     with icons up to 50 px) are made for a phone; a pointer that can aim gets a compact bar, a finger keeps 48 px,
     the least a touch target should be. */
  .host :global(.toolbar-root) {
    --toolbar-button-height: 40px;
    justify-content: flex-start;
    gap: 2px;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }
  .host :global(.toolbar-root .toolbar-button > .toolbar-icon) {
    width: 24px;
    height: 24px;
    max-width: 24px;
    min-width: 0;
    min-height: 0;
  }
  .host :global(.toolbar-root .toolbar-toolContainer > .toolbar-button),
  .host :global(.toolbar-root > button) {
    min-width: 40px;
    width: auto;
    border-radius: 8px;
  }
  .host :global(.toolbar-root .toolbar-showHideDropdownIcon) {
    height: 8px;
  }
  @media (pointer: coarse) {
    .host :global(.toolbar-root) {
      --toolbar-button-height: 48px;
    }
    .host :global(.toolbar-root .toolbar-button > .toolbar-icon) {
      width: 28px;
      height: 28px;
      max-width: 28px;
    }
    .actions button {
      min-height: 44px;
    }
  }
</style>
