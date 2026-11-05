<script lang="ts">
  // One page of a PDF (PA-1, PA-3, PA-5): the picture (a canvas), the text on top of it that can be selected, and the
  // highlights under the text. The viewer mounts only the pages near the screen, so this component renders when it is
  // created or when `scale` changes and gives everything back when it is destroyed.
  //
  // Svelte idioms used here: `$props()` are this component's inputs; `$state` is a value the page re-draws from;
  // `$effect` runs after the DOM is made and again when a `$state` or prop it READ changes (reads after an `await`
  // are not tracked, which is why the values are copied into plain variables first); the function it returns is
  // its cleanup. `untrack` reads or calls something without making the effect depend on it.
  import { untrack } from 'svelte'
  import type { PDFDocumentProxy, PDFPageProxy, RenderTask, TextLayer } from 'pdfjs-dist'
  import { pdfjs } from '../pdf/load'
  import { buildPageText, locate, type PageText } from '../pdf/pageText'
  import { resolveAnchor } from '../pdf/anchor'
  import type { Highlight, Rect } from '../pdf/highlights'

  /** What is drawn over the page: a highlight of the sidecar, or a hit of the find box. */
  export interface Mark {
    key: string
    kind: 'highlight' | 'find' | 'found'
    /** The highlight (kind `highlight`). */
    h: Highlight | null
    /** The range in the page's text (find hits). */
    from: number
    to: number
  }

  /** A box on the page, in CSS pixels from its top left. */
  export interface Box {
    key: string
    kind: Mark['kind']
    color: string
    x: number
    y: number
    w: number
    h: number
    highlight: Highlight | null
  }

  interface Props {
    doc: PDFDocumentProxy
    n: number
    /** CSS pixels per PDF point. */
    scale: number
    /** The size of the page now, also before it is rendered (the viewer keeps the room for it). */
    width: number
    height: number
    marks: Mark[]
    /** The `^id` of a highlight that was just jumped to: its boxes flash. */
    flash: string
    areaMode: boolean
    onSize: (n: number, w: number, h: number) => void
    onText: (n: number, pt: PageText) => void
    /** A highlight was looked for in the text: where it is now (`null`: not found). */
    onPlaced: (h: Highlight, at: { from: number; to: number } | null) => void
    onPick: (h: Highlight, at: { x: number; y: number }) => void
    onArea: (n: number, rect: Rect) => void
  }
  let {
    doc,
    n,
    scale,
    width,
    height,
    marks,
    flash,
    areaMode,
    onSize,
    onText,
    onPlaced,
    onPick,
    onArea,
  }: Props = $props()

  let pageEl: HTMLDivElement
  let canvas: HTMLCanvasElement
  let textEl: HTMLDivElement

  // the text layer of this page once it is drawn; not `$state`: only `ready` tells the effects that it changed
  let layer: { pt: PageText; divs: HTMLElement[]; lens: number[] } | null = null
  let ready = $state(0)
  let boxes = $state<Box[]>([])
  let failed = $state('')

  $effect(() => {
    // the inputs this render depends on; changing any of them runs the cleanup below and renders again
    const scaleNow = scale
    const docNow = doc
    const pageNo = n
    let dead = false
    let task: RenderTask | null = null
    let tl: TextLayer | null = null
    let page: PDFPageProxy | null = null
    layer = null
    failed = ''
    void (async () => {
      try {
        const lib = await pdfjs()
        page = await docNow.getPage(pageNo)
        if (dead) return
        const viewport = page.getViewport({ scale: scaleNow })
        untrack(() => onSize(pageNo, viewport.width / scaleNow, viewport.height / scaleNow))
        // a canvas of the screen's resolution, but not bigger than iOS lets a canvas be
        let ratio = window.devicePixelRatio || 1
        const maxPixels = 16_000_000
        if (viewport.width * viewport.height * ratio * ratio > maxPixels)
          ratio = Math.sqrt(maxPixels / (viewport.width * viewport.height))
        canvas.width = Math.floor(viewport.width * ratio)
        canvas.height = Math.floor(viewport.height * ratio)
        const ctx = canvas.getContext('2d')
        if (!ctx) throw new Error('no canvas')
        task = page.render({
          canvasContext: ctx,
          viewport,
          transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0],
        })
        const content = await page.getTextContent()
        if (dead) return
        pageEl.style.setProperty('--scale-factor', String(scaleNow))
        pageEl.style.setProperty('--total-scale-factor', String(scaleNow))
        // pdf.js draws into this element itself; it is emptied first so a re-render does not stack the text twice
        // eslint-disable-next-line svelte/no-dom-manipulating
        textEl.replaceChildren()
        tl = new lib.TextLayer({ textContentSource: content, container: textEl, viewport })
        await tl.render()
        if (dead) return
        const pt = buildPageText(content.items)
        const divs = tl.textDivs as HTMLElement[]
        // the text layer has one span for each item with a `str`; if it does not, offsets could not be mapped
        if (divs.length === pt.starts.length) {
          divs.forEach((d, i) => (d.dataset.i = String(i)))
          layer = { pt, divs, lens: tl.textContentItemsStr.map((s) => s.length) }
          untrack(() => onText(pageNo, pt))
        } else {
          failed = 'The text of this page could not be matched, so it cannot be highlighted.'
        }
        ready++
        await task.promise
      } catch (e) {
        if (!dead && (e as { name?: string })?.name !== 'RenderingCancelledException')
          failed = (e as Error)?.message ?? 'This page could not be drawn.'
      }
    })()
    return () => {
      dead = true
      task?.cancel()
      tl?.cancel()
      page?.cleanup()
      layer = null
    }
  })

  const COLORS: Record<string, string> = {
    yellow: '#ffd400',
    green: '#4ccf4c',
    blue: '#4aa3ff',
    red: '#ff5c5c',
    purple: '#b178ff',
  }

  /** The boxes of the text between two offsets, from the line boxes the browser gives the text layer's spans. */
  function boxesOfRange(from: number, to: number): { x: number; y: number; w: number; h: number }[] {
    if (!layer || to <= from) return []
    const a = locate(layer.pt, layer.lens, from)
    const b = locate(layer.pt, layer.lens, to)
    if (!a || !b) return []
    const point = (i: number, at: number): [Node, number] => {
      const div = layer!.divs[i]
      return div.firstChild ? [div.firstChild, at] : [div, 0]
    }
    const range = document.createRange()
    range.setStart(...point(a.item, a.at))
    range.setEnd(...point(b.item, b.at))
    const base = pageEl.getBoundingClientRect()
    const out: { x: number; y: number; w: number; h: number }[] = []
    const seen = new Set<string>()
    for (const r of Array.from(range.getClientRects())) {
      if (r.width < 1 || r.height < 1) continue
      const box = { x: r.left - base.left, y: r.top - base.top, w: r.width, h: r.height }
      const id = [box.x, box.y, box.w, box.h].map(Math.round).join(',')
      if (seen.has(id)) continue
      seen.add(id)
      out.push(box)
    }
    return out
  }

  // the boxes: again whenever the marks change or the text layer was drawn again
  $effect(() => {
    void ready
    const list = marks
    const out: Box[] = []
    for (const m of list) {
      if (m.kind === 'highlight' && m.h) {
        const h = m.h
        if (h.kind === 'area' && h.rect) {
          const [x, y, w, hh] = h.rect
          out.push({
            key: m.key,
            kind: m.kind,
            color: COLORS[h.color],
            x: x * width,
            y: y * height,
            w: w * width,
            h: hh * height,
            highlight: h,
          })
          continue
        }
        if (!layer) continue
        const at = resolveAnchor(layer.pt.text, {
          from: h.from,
          to: h.to,
          quote: h.quote,
          context: h.context,
        })
        untrack(() => onPlaced(h, at))
        if (!at) continue
        for (const b of boxesOfRange(at.from, at.to))
          out.push({ key: m.key, kind: m.kind, color: COLORS[h.color], ...b, highlight: h })
      } else if (layer) {
        for (const b of boxesOfRange(m.from, m.to))
          out.push({
            key: m.key,
            kind: m.kind,
            color: m.kind === 'found' ? '#ff8a00' : '#ffe14d',
            ...b,
            highlight: null,
          })
      }
    }
    boxes = out
  })

  /** The offset in this page's text of a point of the text layer (a selection's start or end), or null. */
  export function offsetAt(node: Node, offset: number): number | null {
    if (!layer) return null
    const span = (node.nodeType === Node.TEXT_NODE ? node.parentElement : (node as Element))?.closest(
      '[data-i]',
    ) as HTMLElement | null
    if (span && textEl.contains(span)) {
      const i = Number(span.dataset.i)
      const len = layer.lens[i]
      // inside the text of the span: the offset in it; on the span itself: before (0) or after (1) its text
      const at = node.nodeType === Node.TEXT_NODE ? offset : offset > 0 ? len : 0
      return layer.pt.starts[i] + Math.min(at, len)
    }
    if (node === textEl) {
      // between the spans (a triple click, a drag over the margin): before the child at `offset`
      const child = textEl.childNodes[offset] as HTMLElement | undefined
      const next = child?.closest?.('[data-i]') as HTMLElement | null | undefined
      if (next) return layer.pt.starts[Number(next.dataset.i)]
      return layer.pt.text.length
    }
    return null
  }

  /** The text of the page once it is drawn. */
  export function text(): PageText | null {
    return layer?.pt ?? null
  }

  /** The top of the box with this key, in CSS pixels from the page's top. */
  export function boxTop(key: string): number | null {
    const hit = boxes.find((b) => b.key === key)
    return hit ? hit.y : null
  }

  // a click on a highlight: the boxes are under the text, so the click is found by the point it hit
  function click(e: MouseEvent) {
    if (!window.getSelection()?.isCollapsed) return
    const base = pageEl.getBoundingClientRect()
    const x = e.clientX - base.left
    const y = e.clientY - base.top
    const hit = boxes.find((b) => b.highlight && x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h)
    if (hit?.highlight) onPick(hit.highlight, { x: e.clientX, y: e.clientY })
  }

  // area highlight: drag a rectangle (PA-5)
  let drag = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null)
  function down(e: PointerEvent) {
    if (!areaMode || e.button > 0) return
    const base = pageEl.getBoundingClientRect()
    const x = Math.min(Math.max(e.clientX - base.left, 0), width)
    const y = Math.min(Math.max(e.clientY - base.top, 0), height)
    drag = { x0: x, y0: y, x1: x, y1: y }
    e.preventDefault()
  }
  function move(e: PointerEvent) {
    if (!drag) return
    const base = pageEl.getBoundingClientRect()
    drag.x1 = Math.min(Math.max(e.clientX - base.left, 0), width) // cut to the page
    drag.y1 = Math.min(Math.max(e.clientY - base.top, 0), height)
  }
  function up() {
    if (!drag) return
    const d = drag
    drag = null
    const x = Math.min(d.x0, d.x1)
    const y = Math.min(d.y0, d.y1)
    const w = Math.abs(d.x1 - d.x0)
    const h = Math.abs(d.y1 - d.y0)
    if (w < 8 || h < 8) return
    onArea(n, [x / width, y / height, w / width, h / height])
  }
</script>

<!-- the drag goes on outside the page too (the rectangle is cut to the page), so the window listens -->
<svelte:window onpointermove={move} onpointerup={up} onpointercancel={() => (drag = null)} />

<div
  class="page"
  bind:this={pageEl}
  data-n={n}
  data-testid="pdf-page"
  style:width="{width}px"
  style:height="{height}px"
>
  <canvas bind:this={canvas} style:width="{width}px" style:height="{height}px"></canvas>
  <div class="marks" aria-hidden="true">
    {#each boxes as b, i (b.key + ':' + i)}
      <div
        class="box {b.kind}"
        class:area={b.highlight?.kind === 'area'}
        class:flash={flash !== '' && b.highlight?.uid === flash}
        data-testid={b.kind === 'highlight' ? 'pdf-highlight' : 'pdf-find-hit'}
        data-uid={b.highlight?.uid ?? ''}
        data-color={b.highlight?.color ?? ''}
        style:left="{b.x}px"
        style:top="{b.y}px"
        style:width="{b.w}px"
        style:height="{b.h}px"
        style:--c={b.color}
      ></div>
    {/each}
  </div>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="textLayer" bind:this={textEl} onclick={click}></div>
  {#if areaMode}
    <!-- a layer over the page that takes the drag, so the text under it is not selected -->
    <div class="catcher" data-testid="pdf-area-layer" onpointerdown={down} role="presentation">
      {#if drag}
        <div
          class="rubber"
          style:left="{Math.min(drag.x0, drag.x1)}px"
          style:top="{Math.min(drag.y0, drag.y1)}px"
          style:width="{Math.abs(drag.x1 - drag.x0)}px"
          style:height="{Math.abs(drag.y1 - drag.y0)}px"
        ></div>
      {/if}
    </div>
  {/if}
  {#if failed}<p class="failed" role="status">{failed}</p>{/if}
</div>

<style>
  .page {
    position: relative;
    background: #fff;
    box-shadow: 0 1px 6px rgb(0 0 0 / 0.3);
    margin: 0 auto;
    flex: none;
  }
  canvas {
    position: absolute;
    inset: 0;
  }
  .marks {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .box {
    position: absolute;
    background: var(--c);
    mix-blend-mode: multiply;
    opacity: 0.45;
    border-radius: 2px;
  }
  .box.flash {
    animation: flash 0.5s ease-in-out 3;
  }
  @keyframes flash {
    50% {
      opacity: 1;
      outline: 3px solid var(--c);
    }
  }
  .box.found {
    opacity: 0.7;
    outline: 2px solid #ff8a00;
  }
  .box.area {
    background: transparent;
    mix-blend-mode: normal;
    opacity: 1;
    outline: 3px solid var(--c);
    outline-offset: -1px;
    border-radius: 3px;
  }
  /* the text on top of the picture: invisible, but it can be selected (the same rules as pdf.js's own viewer) */
  .textLayer {
    position: absolute;
    inset: 0;
    overflow: clip;
    line-height: 1;
    text-size-adjust: none;
    forced-color-adjust: none;
    transform-origin: 0 0;
    -webkit-user-select: text;
    user-select: text;
  }
  .textLayer :global(:is(span, br)) {
    color: transparent;
    position: absolute;
    white-space: pre;
    cursor: text;
    transform-origin: 0% 0%;
  }
  .textLayer :global(span.markedContent) {
    top: 0;
    height: 0;
  }
  .textLayer :global(::selection) {
    background: rgb(0 90 255 / 0.3);
  }
  .catcher {
    position: absolute;
    inset: 0;
    z-index: 5;
    cursor: crosshair;
    touch-action: none;
    background: rgb(42 125 225 / 0.04);
  }
  .rubber {
    position: absolute;
    border: 2px dashed var(--accent, #2a7de1);
    background: rgb(42 125 225 / 0.12);
  }
  .failed {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    margin: 0;
    padding: 0.4em 0.8em;
    background: #fff3cd;
    color: #5c4400;
    font-size: 0.8em;
  }
</style>
