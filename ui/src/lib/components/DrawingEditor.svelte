<script lang="ts">
  // The drawing editor (DR-1 to DR-6): js-draw in a full-screen dialog. js-draw is a plain DOM library, not a Svelte
  // one, so it is created in `onMount` into a `<div>` (`bind:this` gives us the element) and taken apart when the
  // component goes away; what it knows is read when a button is pressed, not tracked as state.
  import { onMount } from 'svelte'
  import Editor, {
    BackgroundComponent,
    BackgroundComponentBackgroundType,
    Color4,
    Erase,
    EditorEventType,
    Mat33,
    PanZoomMode,
    PanZoomTool,
    PenTool,
    Rect2,
    SVGLoader,
    SelectionMode,
    SelectionTool,
    Viewport,
    makeFreehandLineBuilder,
    makePressureSensitiveFreehandLineBuilder,
    uniteCommands,
    type AbstractComponent,
  } from 'js-draw'
  import 'js-draw/bundledStyles'
  import { backend } from '../api'
  import { app, toast, errorMessage } from '../stores/app.svelte'
  import {
    drawingMarkdown,
    finishDrawing,
    type DrawingMode,
    type DrawingOpen,
    type DrawingResult,
  } from '../commands/drawing'
  import { makeRawInkBuilder } from '../rawInk'
  import { MakeSpaceTool } from '../makeSpaceTool'
  import { DraftSaver, draftKey, idbStore, type Draft, type DraftStore } from '../drawing/draft'
  import { copyPng, exportName, pngBlob, saveFile, svgBlob } from '../drawing/exportImage'
  import { loadPrefs, savePrefs } from '../drawing/prefs'
  import { loadPresets, presetOf, samePen, savePresets } from '../drawing/presets'
  import { percent, zoomed } from '../drawing/view'
  import DrawingMenu, { type MenuItem } from './DrawingMenu.svelte'
  import DrawingPresets from './DrawingPresets.svelte'

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
  // '1': a stylus was seen on this device (or "Pen only" was switched on): from then on one finger moves the canvas, so
  // a resting hand does not draw. '0': "Pen only" was switched off by hand, a stylus does not switch it on again.
  const STYLUS_KEY = 'notatnik.drawing.stylus'
  // the lasso is the selection of this editor (DR-16): made the default once per device, the person's choice after
  const LASSO_KEY = 'notatnik.drawing.lasso'

  let host: HTMLDivElement | undefined = $state()
  let editor: Editor | undefined
  let toolbar: ReturnType<Editor['addToolbar']> | undefined
  // "Make space" (DR-12): whether the tool is on, and where its line is while dragging (client pixels, null: none).
  // Both are `$state` because the template shows them; the tool itself is a plain js-draw object.
  let spaceOn = $state(false)
  let spaceLine = $state<{ axis: 'x' | 'y'; at: number } | null>(null)
  // the undo depth right after the drawing was loaded: the drawing is changed when it differs
  let baseline = 0
  // changes that are not in the undo history (a draft that was restored, a copy that was used)
  let forceDirty = false
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

  // the pen presets, the active one, Pen only, the zoom label, and the device preferences (DR-15..DR-21)
  let presets = $state(loadPresets())
  let activePreset = $state(-1)
  let penOnly = $state(false)
  let zoomLabel = $state('100 %')
  let prefs = $state(loadPrefs())
  let focus = $state(false)
  // what the Background menu shows: read from the drawing when the menu opens
  let bgKind = $state<BgKind>('medium')
  // a draft of this drawing that was kept (DR-13), and the copies a sync left of it (DR-23)
  let draft = $state<Draft | null>(null)
  let copies = $state<string[]>([])
  // copies whose strokes are in this drawing now: they go to the trash when it is saved
  let consumed: string[] = []
  let drafts: DraftStore | undefined
  let saver: DraftSaver | undefined

  type BgKind = 'blank' | 'small' | 'medium' | 'large'
  /** The cell of the grid, in canvas units, for each size. Medium is what a new drawing gets. */
  const GRID: Record<Exclude<BgKind, 'blank'>, number> = { small: 12.5, medium: 25, large: 50 }

  // `getAllComponents` leaves the sheet out (it is a background component), so an empty drawing has none
  const isEmpty = () => (editor?.image.getAllComponents().length ?? 0) === 0
  const strokesChanged = () => forceDirty || (!!editor && editor.history.undoStackSize !== baseline)
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

  /** Pen only (DR-17): fingers move the canvas instead of drawing. Remembered on this device. */
  function setPenOnly(on: boolean) {
    penOnly = on
    if (editor) fingerMovesCanvas(editor, on)
    try {
      localStorage.setItem(STYLUS_KEY, on ? '1' : '0')
    } catch {
      /* it is just noticed again next time */
    }
  }

  /** The first time a stylus touches the canvas, fingers stop drawing, now and in the next sessions: unless the
   * person switched Pen only off. */
  function noticeStylus(e: PointerEvent) {
    if (e.pointerType !== 'pen' || !editor || penOnly) return
    try {
      if (localStorage.getItem(STYLUS_KEY) === '0') return
    } catch {
      /* no storage: the stylus counts */
    }
    setPenOnly(true)
  }

  /** The pen that writes by hand (the one that keeps every point), and the pen that is in use now. */
  const handwritingPen = (ed: Editor) =>
    ed.toolController.getMatchingTools(PenTool).find((p) => p.getStrokeFactory() === makeRawInkBuilder)
  const enabledPen = (ed: Editor) => ed.toolController.getMatchingTools(PenTool).find((p) => p.isEnabled())

  /** Which preset is the pen in use now? (Called when a tool changes, so the highlight follows the toolbar too.) */
  function syncActivePreset() {
    const pen = editor && enabledPen(editor)
    activePreset = pen
      ? presets.findIndex((p) => samePen(p, presetOf(pen.getColor(), pen.getThickness())))
      : -1
  }

  /** A preset sets colour, width and opacity of the pen in use, or of the handwriting pen when no pen is in use. */
  function applyPreset(i: number) {
    const ed = editor
    const pen = ed && (enabledPen(ed) ?? handwritingPen(ed))
    if (!ed || !pen) return
    const p = presets[i]
    pen.setColor(Color4.fromHex(p.color).withAlpha(p.alpha))
    pen.setThickness(p.width)
    pen.setEnabled(true)
    syncActivePreset()
    ed.focus()
  }

  /** The pen as it is now goes into slot `i`. */
  function storePreset(i: number) {
    const ed = editor
    const pen = ed && (enabledPen(ed) ?? handwritingPen(ed))
    if (!pen) return
    presets[i] = presetOf(pen.getColor(), pen.getThickness())
    savePresets(presets)
    syncActivePreset()
    toast(`Pen ${i + 1} stored.`)
  }

  /** Hold a pen still at the end of a stroke and it snaps to a line or a clean shape (DR-18): off, handwriting stays
   * as written (DR-3). */
  function applySnap(on: boolean) {
    if (!editor) return
    for (const pen of editor.toolController.getMatchingTools(PenTool)) pen.setStrokeAutocorrectEnabled(on)
  }

  /** The zoom (DR-19): a step in or out around the middle of what is shown, the label, back to 100 %, and fit. */
  function zoomStep(dir: 1 | -1) {
    const ed = editor
    if (!ed) return
    const s = ed.viewport.getScaleFactor()
    const k = zoomed(s, dir) / s
    void ed.dispatchNoAnnounce(
      Viewport.transformBy(Mat33.scaling2D(k, ed.viewport.visibleRect.center)),
      false,
    )
  }
  function zoomReset() {
    const ed = editor
    if (!ed) return
    const s = ed.viewport.getScaleFactor()
    void ed.dispatchNoAnnounce(
      Viewport.transformBy(Mat33.scaling2D(1 / s, ed.viewport.visibleRect.center)),
      false,
    )
  }
  /** What has been drawn, without the sheet it is drawn on (null: nothing). */
  function contentRect(ed: Editor): Rect2 | null {
    const boxes = ed.image.getAllComponents().map((c: AbstractComponent) => c.getBBox())
    return boxes.length ? Rect2.union(...boxes) : null
  }
  function fitView() {
    const ed = editor
    const r = ed && contentRect(ed)
    if (!ed || !r) return zoomReset()
    void ed.dispatchNoAnnounce(
      ed.viewport.zoomTo(r.grownBy(Math.max(20, r.maxDimension * 0.05)), true, true),
      false,
    )
  }

  /** The background (DR-20): read from the drawing, and replaced as one undo step. */
  function backgroundKind(ed: Editor): BgKind {
    const bg = ed.image
      .getBackgroundComponents()
      .find((c): c is BackgroundComponent => c instanceof BackgroundComponent)
    if (!bg || bg.getBackgroundType() !== BackgroundComponentBackgroundType.Grid) return 'blank'
    const size = bg.getGridSize()
    const kinds = Object.keys(GRID) as Exclude<BgKind, 'blank'>[]
    return kinds.reduce(
      (best, k) => (Math.abs(GRID[k] - size) < Math.abs(GRID[best] - size) ? k : best),
      'medium',
    )
  }
  async function setBackground(kind: BgKind) {
    const ed = editor
    if (!ed) return
    const old = ed.image.getBackgroundComponents()
    const next =
      kind === 'blank'
        ? new BackgroundComponent(BackgroundComponentBackgroundType.SolidColor, Color4.white)
        : BackgroundComponent.ofGrid(Color4.white, GRID[kind])
    await ed.dispatch(uniteCommands([...old.map((c) => new Erase([c])), ed.image.addElement(next)], 100))
    bgKind = kind
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
    const space = new MakeSpaceTool(ed, (line) => (spaceLine = line))
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
    applySnap(prefs.snapOnHold)
    // the lasso, not the rectangle, is how handwriting is selected (DR-16): once per device, then it is the choice
    try {
      if (!localStorage.getItem(LASSO_KEY)) {
        ed.toolController.getMatchingTools(SelectionTool).forEach((t) => t.modeValue.set(SelectionMode.Lasso))
        localStorage.setItem(LASSO_KEY, '1')
      }
    } catch {
      /* no storage: the toolbar's own choice stays */
    }
    try {
      penOnly = localStorage.getItem(STYLUS_KEY) === '1'
    } catch {
      /* no storage: fingers draw until a stylus is seen */
    }
    if (penOnly) fingerMovesCanvas(ed, true)
    // the toolbar changes the pen, the viewport changes the zoom: the buttons of this editor follow both
    const subs = [
      ed.notifier.on(EditorEventType.ToolUpdated, syncActivePreset),
      ed.notifier.on(EditorEventType.ToolEnabled, syncActivePreset),
      ed.notifier.on(
        EditorEventType.ViewportChanged,
        () => (zoomLabel = percent(ed.viewport.getScaleFactor())),
      ),
    ]
    syncActivePreset()
    // a draft is written when the page is hidden: an iPad stops an app that is in the background whenever it likes
    const hide = () => {
      if (document.visibilityState === 'hidden') void saver?.flush()
    }
    document.addEventListener('visibilitychange', hide)
    window.addEventListener('pagehide', hide)
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
      zoomLabel = percent(ed.viewport.getScaleFactor())
      // a draft of what was being drawn when the app stopped, and the copies of this drawing a sync left (DR-13, DR-23)
      drafts = idbStore()
      saver = new DraftSaver(drafts, draftKey(target), () => (dirty() ? serialize(ed) : null))
      try {
        const found = await drafts.get(draftKey(target))
        if (found?.svg) draft = found
      } catch {
        /* no storage: nothing was kept */
      }
      for (const e of [EditorEventType.CommandDone, EditorEventType.CommandUndone])
        subs.push(ed.notifier.on(e, () => saver?.touch()))
      if (target.path) {
        try {
          copies = (await backend.call('drawing_copies', { path: target.path })).items
        } catch {
          /* an older backend, or none: no copies are offered */
        }
      }
    })()
    return () => {
      for (const s of subs) s.remove()
      document.removeEventListener('visibilitychange', hide)
      window.removeEventListener('pagehide', hide)
      host?.removeEventListener('pointerdown', noticeStylus, true)
      toolbar?.remove()
      ed.remove()
      editor = undefined
    }
  })

  const serialize = (ed: Editor) => new XMLSerializer().serializeToString(ed.toSVG())

  /** Closes the editor: the draft goes (what it kept is saved or thrown away now), the pens are remembered. */
  async function leave(result: DrawingResult | null) {
    await saver?.clear()
    rememberTools()
    finishDrawing(result)
  }

  /** Puts a drawing from SVG text in place of the one on the canvas: a draft, or the other device's copy. */
  async function replaceWith(svg: string) {
    const ed = editor
    if (!ed) return
    // `loadFromSVG` adds to what is on the canvas: the strokes that are there go first (not into the undo history)
    await ed.dispatchNoAnnounce(new Erase(ed.image.getAllComponents()), false)
    await ed.loadFromSVG(svg, true)
    void ed.dispatch(ed.image.setAutoresizeEnabled(true), false)
    forceDirty = true
    saver?.touch()
    ed.focus()
  }

  async function restoreDraft() {
    const d = draft
    draft = null
    if (d) await replaceWith(d.svg)
  }
  async function dropDraft() {
    draft = null
    try {
      await drafts?.delete(draftKey(target))
    } catch {
      /* it was not there */
    }
    editor?.focus()
  }

  /** A stroke as a key that is the same for the same stroke loaded twice: what it is made of, without its id. */
  function componentKey(c: AbstractComponent): string {
    const { name, data } = c.serialize()
    return JSON.stringify([name, data])
  }

  /** The strokes of the other device's copy are added to this drawing; the ones both versions have stay single. Nothing
   * is lost, and what one side had erased comes back, which is the price of not asking about every stroke. */
  async function addCopyStrokes(copy: string) {
    const ed = editor
    if (!ed) return
    try {
      const text = (await backend.call('read_drawing', { path: copy })).value
      const have = new Set(ed.image.getAllComponents().map(componentKey))
      const added: AbstractComponent[] = []
      await SVGLoader.fromString(text, true).start(
        (c) => {
          if (!c.isBackground() && !have.has(componentKey(c))) added.push(c)
        },
        () => {},
      )
      if (added.length)
        await ed.dispatch(
          uniteCommands(
            added.map((c) => ed.image.addElement(c)),
            100,
          ),
        )
      forceDirty = true
      consumed.push(copy)
      copies = copies.filter((c) => c !== copy)
      toast(
        added.length
          ? `Added ${added.length} strokes from the other version.`
          : 'The other version has nothing more.',
      )
      saver?.touch()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function useCopy(copy: string) {
    try {
      await replaceWith((await backend.call('read_drawing', { path: copy })).value)
      consumed.push(copy)
      copies = copies.filter((c) => c !== copy)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function keepMine(copy: string) {
    try {
      await backend.call('discard_drawing_copy', { path: copy })
      copies = copies.filter((c) => c !== copy)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
    editor?.focus()
  }
  /** The date and time of a copy, from its name (`… (conflicted copy 2026-10-03 120000).svg`). */
  const copyWhen = (p: string) => /\(conflicted copy ([^)]*)\)/.exec(p)?.[1] ?? ''

  /** The drawing outside the app (DR-22). */
  async function copyAsPng() {
    if (!editor) return
    if (await copyPng(editor)) toast('Copied the drawing as a picture.')
    else toast('This window cannot copy a picture. Use Save as PNG.', 'error')
  }
  async function saveAs(kind: 'png' | 'svg') {
    const ed = editor
    if (!ed) return
    try {
      const blob = kind === 'png' ? await pngBlob(ed) : svgBlob(ed)
      const where = await saveFile(blob, exportName(name, kind))
      if (where && where !== 'download') toast(`Saved to ${where}.`)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  /** The "More" menu: what is not needed at every stroke. Built again when it opens, so it shows the state now. */
  function menuItems(): MenuItem[] {
    const pick = (k: BgKind, label: string): MenuItem => ({
      kind: 'radio',
      label,
      checked: bgKind === k,
      testid: `drawing-bg-${k}`,
      onselect: () => void setBackground(k),
    })
    return [
      { kind: 'heading', label: 'Background' },
      pick('small', 'Grid, small'),
      pick('medium', 'Grid, medium'),
      pick('large', 'Grid, large'),
      pick('blank', 'Blank'),
      { kind: 'sep' },
      { kind: 'sep' },
      {
        kind: 'check',
        label: 'Snap shapes when a pen is held still',
        checked: prefs.snapOnHold,
        testid: 'drawing-snap',
        onselect: () => {
          prefs.snapOnHold = !prefs.snapOnHold
          savePrefs(prefs)
          applySnap(prefs.snapOnHold)
        },
      },
      {
        kind: 'check',
        label: 'Toolbar below the canvas',
        checked: prefs.toolbarBottom,
        testid: 'drawing-toolbar-bottom',
        onselect: () => {
          prefs.toolbarBottom = !prefs.toolbarBottom
          savePrefs(prefs)
        },
      },
      {
        kind: 'item',
        label: 'Focus: hide everything but the canvas',
        testid: 'drawing-focus',
        onselect: () => (focus = true),
      },
      { kind: 'sep' },
      {
        kind: 'item',
        label: 'Copy as a picture',
        testid: 'drawing-copy-png',
        onselect: () => void copyAsPng(),
      },
      { kind: 'item', label: 'Save as PNG…', testid: 'drawing-save-png', onselect: () => void saveAs('png') },
      { kind: 'item', label: 'Save as SVG…', testid: 'drawing-save-svg', onselect: () => void saveAs('svg') },
    ]
  }

  /** Done: a new drawing that is empty, or an old one with nothing changed, stores nothing. The file is written when
   * the strokes changed; a new name or form only rewrites the reference in the block. */
  async function done() {
    const ed = editor
    if (!ed || saving || loading) return
    if (isNew ? isEmpty() : !strokesChanged() && !metaChanged()) {
      await leave(null)
      return
    }
    saving = true
    try {
      let path = target.path
      let markdown = ''
      if (isNew || strokesChanged()) {
        const svg = serialize(ed)
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
      // the copies whose strokes are in the drawing now are not needed any more
      for (const c of consumed) await backend.call('discard_drawing_copy', { path: c }).catch(() => undefined)
      await leave({ path: path ?? '', markdown })
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
      await leave({ path, markdown: '', deleted: true })
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
    void leave(null)
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
    class:focus
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
      <DrawingPresets {presets} active={activePreset} onApply={applyPreset} onStore={storePreset} />
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
        <button type="button" class="danger" data-testid="drawing-discard" onclick={() => void leave(null)}
          >Discard</button
        >
      </div>
    {/if}
    {#if draft}
      <!-- DR-13: what was being drawn when the app stopped -->
      <div class="confirm banner" role="status" data-testid="drawing-draft-banner">
        <span
          >A drawing that was not saved was kept ({new Date(draft.savedAt).toLocaleString()}). Restore it?</span
        >
        <button type="button" data-testid="drawing-draft-restore" onclick={restoreDraft}>Restore</button>
        <button type="button" data-testid="drawing-draft-discard" onclick={dropDraft}>Discard it</button>
      </div>
    {/if}
    {#if copies.length}
      <!-- DR-23: this drawing was changed on two devices; the sync kept the other version beside it -->
      <div class="confirm banner" role="status" data-testid="drawing-copy-banner">
        <span
          >This drawing was changed on two devices. The other version ({copyWhen(copies[0])}) was saved as a
          copy{copies.length > 1 ? `, and ${copies.length - 1} more` : ''}.</span
        >
        <button type="button" data-testid="drawing-copy-add" onclick={() => void addCopyStrokes(copies[0])}
          >Add its strokes</button
        >
        <button type="button" data-testid="drawing-copy-use" onclick={() => void useCopy(copies[0])}
          >Use that version</button
        >
        <button type="button" data-testid="drawing-copy-keep" onclick={() => void keepMine(copies[0])}
          >Keep this one</button
        >
      </div>
    {/if}
    <div class="host" bind:this={host} class:loading class:bottom={prefs.toolbarBottom}></div>
    <!-- the help sits under the canvas, out of the way of the controls above, with what is needed less often -->
    <div class="bar">
      <p class="hint" data-testid="drawing-hint">
        {#if spaceOn}Make space: press where the space should start and drag down or up, or sideways;
          everything below or beside it follows. Pick a pen to stop{:else}The pen draws; once a stylus has
          been used a finger moves the canvas and two fingers zoom{/if}
      </p>
      <div class="zoom" role="group" aria-label="Zoom">
        <button
          type="button"
          aria-label="Zoom out"
          title="Zoom out"
          data-testid="drawing-zoom-out"
          onclick={() => zoomStep(-1)}>−</button
        >
        <button type="button" class="pct" title="Back to 100 %" data-testid="drawing-zoom" onclick={zoomReset}
          >{zoomLabel}</button
        >
        <button
          type="button"
          aria-label="Zoom in"
          title="Zoom in"
          data-testid="drawing-zoom-in"
          onclick={() => zoomStep(1)}>+</button
        >
        <button type="button" title="Show the whole drawing" data-testid="drawing-fit" onclick={fitView}
          >Fit</button
        >
      </div>
      <button
        type="button"
        class="toggle"
        class:on={penOnly}
        aria-pressed={penOnly}
        title="Fingers move the canvas instead of drawing; the pen and the mouse draw"
        data-testid="drawing-pen-only"
        onclick={() => setPenOnly(!penOnly)}>Pen only</button
      >
      <DrawingMenu items={menuItems()} onopen={() => editor && (bgKind = backgroundKind(editor))} />
    </div>
    {#if focus}
      <!-- DR-21: nothing but the canvas; these two stay in reach -->
      <div class="focus-pill" data-testid="drawing-focus-pill">
        <button type="button" onclick={() => (focus = false)} data-testid="drawing-focus-exit"
          >Show controls</button
        >
        <button type="button" class="primary" onclick={done} disabled={loading || saving}>Done</button>
      </div>
    {/if}
    {#if spaceLine !== null}
      <!-- the line of "Make space": fixed on the page where the tool reports it, across the page either way -->
      <div
        class="space-line {spaceLine.axis}"
        style:top={spaceLine.axis === 'y' ? `${spaceLine.at}px` : null}
        style:left={spaceLine.axis === 'x' ? `${spaceLine.at}px` : null}
        data-testid="space-line"
        aria-hidden="true"
      ></div>
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
    pointer-events: none;
    z-index: 70;
  }
  .space-line.y {
    left: 0;
    right: 0;
    height: 0;
    border-top: 2px dashed var(--accent, #2563eb);
  }
  .space-line.x {
    top: 0;
    bottom: 0;
    width: 0;
    border-left: 2px dashed var(--accent, #2563eb);
  }
  /* the line under the canvas: the help, the zoom, Pen only and the More menu */
  .bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.6em;
    padding: 0.3em 0.9em;
    border-top: 1px solid var(--border);
  }
  .hint {
    margin: 0;
    flex: 1;
    min-width: 14em;
    color: var(--muted);
    font-size: 0.8em;
  }
  .zoom {
    display: flex;
    gap: 2px;
  }
  .zoom button,
  .toggle {
    font: inherit;
    font-size: 0.85em;
    padding: 0.2em 0.6em;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .zoom .pct {
    min-width: 4.2em;
    font-variant-numeric: tabular-nums;
  }
  .toggle.on {
    background: var(--chip);
    border-color: var(--accent);
    font-weight: 600;
  }
  .banner span {
    flex: 1;
  }
  .host.bottom :global(.imageEditorContainer) {
    flex-direction: column;
  }
  .host.bottom :global(.toolbar-root) {
    border-bottom: 0;
    border-top: 1px solid var(--border);
  }
  /* the focus mode: only the canvas, and two buttons in the corner */
  .dialog.focus header,
  .dialog.focus .bar,
  .dialog.focus .banner {
    display: none;
  }
  .focus-pill {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 6px;
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 2px 10px rgb(0 0 0 / 0.2);
    z-index: 75;
  }
  .focus-pill button {
    font: inherit;
    padding: 0.3em 0.9em;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .focus-pill .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .dialog {
    position: relative;
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
    .actions button,
    .zoom button,
    .toggle {
      min-height: 44px;
    }
  }
</style>
