<script lang="ts">
  // The PDF viewer (PA-1 to PA-8): a PDF of the graph in a full-screen view, with highlights and comments kept as blocks of
  // the PDF's sidecar page (`docs/phases/pdf-annotation.md`). The PDF itself is never written.
  //
  // Svelte idioms used here: `$state` is a value the page re-draws from; `$derived` is a value computed from other
  // state (never write state inside one); `$effect` runs after the DOM exists and again when state it read changes;
  // `bind:this` gives the element or child component; `untrack` reads without making an effect depend on it.
  import { onMount, tick, untrack } from 'svelte'
  import type { PDFDocumentProxy } from 'pdfjs-dist'
  import { backend } from '../api'
  import { app, errorMessage, toast } from '../stores/app.svelte'
  import { finishPdf } from '../commands/pdf'
  import { pageHash } from '../route'
  import { openDocument, PdfError } from '../pdf/load'
  import { buildPageText, type PageText } from '../pdf/pageText'
  import { CONTEXT_CHARS, contextOf, quoteOf } from '../pdf/anchor'
  import {
    areaHighlightBlock,
    COLORS,
    textHighlightBlock,
    type Color,
    type Highlight,
    type Rect,
  } from '../pdf/highlights'
  import PdfPage, { type Mark } from './PdfPage.svelte'
  import AnnotationPanel from './AnnotationPanel.svelte'
  import HighlightPopup from './HighlightPopup.svelte'
  import SelectionBar from './SelectionBar.svelte'
  import { COLOR_HEX } from '../annotate/colors'
  import { Sidecar, copyText, quoteBlock, refOf } from '../annotate/sidecar.svelte'

  // `app.pdf` is set when the viewer opens and stays the same until it closes: a snapshot is enough
  const target = untrack(() => app.pdf)!
  const fileName = target.path.split('/').pop() ?? target.path

  const GAP = 12
  const PAD = 16

  // ---------------------------------------------------------------- the document
  let doc = $state<PDFDocumentProxy | null>(null)
  let status = $state<'loading' | 'ready' | 'error'>('loading')
  let message = $state('')
  let numPages = $state(0)
  let password = $state<{ wrong: boolean; answer: (pw: string | null) => void } | null>(null)
  let passwordText = $state('')

  // the size of every page in PDF points; the first page stands in for those not drawn yet
  const dims = new Map<number, { w: number; h: number }>()
  let rev = $state(0) // bumped when a size is learned, so `tops` is computed again
  const texts = new Map<number, PageText>() // the text of the pages seen so far

  onMount(() => {
    let dead = false
    void (async () => {
      try {
        const d = await openDocument(backend.assetUrl(target.path), (wrong) => {
          return new Promise((resolve) => {
            password = {
              wrong,
              answer: (pw) => {
                password = null
                passwordText = ''
                resolve(pw)
              },
            }
          })
        })
        if (dead) {
          void d.destroy()
          return
        }
        const first = await d.getPage(1)
        const vp = first.getViewport({ scale: 1 })
        dims.set(1, { w: vp.width, h: vp.height })
        doc = d
        numPages = d.numPages
        status = 'ready'
        await sc.load()
      } catch (e) {
        if (dead) return
        status = 'error'
        message =
          e instanceof PdfError && e.failure === 'password' ? 'This PDF needs a password.' : errorMessage(e)
      }
    })()
    return () => {
      dead = true
      void doc?.destroy()
    }
  })

  // ---------------------------------------------------------------- layout and scrolling
  let scroller = $state<HTMLDivElement | null>(null)
  let viewW = $state(0)
  let viewH = $state(0)
  let zoom = $state(1) // 1 is the width of the window
  let current = $state(1)
  let mountFrom = $state(1)
  let mountTo = $state(1)

  const pts = (n: number) => dims.get(n) ?? dims.get(1) ?? { w: 612, h: 792 }
  const fit = $derived(viewW > 0 && dims.get(1) ? (viewW - 2 * PAD) / dims.get(1)!.w : 1)
  const scale = $derived(Math.max(0.05, fit * zoom))
  // the top of every page (in CSS pixels from the top of the scrolled content), and the height of it all
  const layout = $derived.by(() => {
    void rev
    const tops: number[] = []
    let y = PAD
    for (let n = 1; n <= numPages; n++) {
      tops.push(y)
      y += pts(n).h * scale + GAP
    }
    return { tops, total: y + PAD - GAP }
  })

  function pageAt(y: number): number {
    const { tops } = layout
    let lo = 0
    let hi = tops.length - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (tops[mid] <= y) lo = mid
      else hi = mid - 1
    }
    return lo + 1
  }

  function updateVisible() {
    if (!scroller || numPages === 0) return
    const top = scroller.scrollTop
    current = pageAt(top + viewH / 3)
    // the pages on the screen and one screen of them above and below: a few at a time, however long the PDF is
    mountFrom = pageAt(Math.max(0, top - viewH))
    mountTo = pageAt(top + 2 * viewH)
  }

  let raf = 0
  function onScroll() {
    if (raf) return
    raf = requestAnimationFrame(() => {
      raf = 0
      updateVisible()
    })
  }

  $effect(() => {
    // when the window, the zoom or the page sizes change, which pages are on the screen changes too
    void layout
    void viewH
    untrack(updateVisible)
  })

  // the first position (the page of the link, or a highlight), once the window has its size and the sidecar is known:
  // before that the page sizes are not final and the position would move
  let placed = false
  $effect(() => {
    if (status === 'ready' && sc.loaded && viewW > 0 && scroller && !placed) {
      placed = true
      untrack(() => void openAt())
    }
  })

  function goToPage(n: number) {
    if (!scroller) return
    const p = Math.min(Math.max(1, Math.round(n)), numPages)
    scroller.scrollTop = layout.tops[p - 1] - PAD
    current = p
    updateVisible()
  }

  async function setZoom(z: number) {
    if (!scroller) return
    const before = scroller.scrollTop / Math.max(1, layout.total)
    zoom = Math.min(5, Math.max(0.25, z))
    await tick()
    scroller.scrollTop = before * layout.total
    updateVisible()
  }
  const zoomBy = (f: number) => setZoom(zoom * f)

  // Ctrl+wheel and a pinch zoom the pages; the listeners are added by hand because `wheel` must not be passive
  $effect(() => {
    const el = scroller
    if (!el) return
    const wheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return
      e.preventDefault()
      void zoomBy(e.deltaY < 0 ? 1.1 : 1 / 1.1)
    }
    let start = 0
    let startZoom = 1
    const dist = (t: TouchList) => Math.hypot(t[0].clientX - t[1].clientX, t[0].clientY - t[1].clientY)
    const touchStart = (e: TouchEvent) => {
      if (e.touches.length === 2) {
        start = dist(e.touches)
        startZoom = untrack(() => zoom)
      }
    }
    const touchEnd = (e: TouchEvent) => {
      if (start && e.touches.length < 2) {
        const ratio = lastDist / start
        start = 0
        if (Math.abs(ratio - 1) > 0.05) void setZoom(startZoom * ratio)
      }
    }
    let lastDist = 0
    const touchMove = (e: TouchEvent) => {
      if (start && e.touches.length === 2) {
        lastDist = dist(e.touches)
        e.preventDefault()
      }
    }
    el.addEventListener('wheel', wheel, { passive: false })
    el.addEventListener('touchstart', touchStart, { passive: true })
    el.addEventListener('touchmove', touchMove, { passive: false })
    el.addEventListener('touchend', touchEnd, { passive: true })
    return () => {
      el.removeEventListener('wheel', wheel)
      el.removeEventListener('touchstart', touchStart)
      el.removeEventListener('touchmove', touchMove)
      el.removeEventListener('touchend', touchEnd)
    }
  })

  function onSize(n: number, w: number, h: number) {
    const old = dims.get(n)
    if (old && Math.abs(old.w - w) < 0.01 && Math.abs(old.h - h) < 0.01) return
    dims.set(n, { w, h })
    rev++
  }
  function onText(n: number, pt: PageText) {
    texts.set(n, pt)
  }

  /** The text of any page, also one that is not drawn (for the find box and a selection over several pages). */
  async function textOf(n: number): Promise<PageText | null> {
    const seen = texts.get(n)
    if (seen) return seen
    if (!doc) return null
    const content = await (await doc.getPage(n)).getTextContent()
    const pt = buildPageText(content.items)
    texts.set(n, pt)
    return pt
  }

  // ---------------------------------------------------------------- the sidecar and its highlights
  const sc = new Sidecar(target.path)

  /** A text highlight was looked for in the text of its page: the numbers are corrected when the quote moved. */
  function onPlaced(h: Highlight, at: { from: number; to: number } | null) {
    sc.mark(h, at !== null)
    if (at && (at.from !== h.from || at.to !== h.to))
      sc.correct(h, { 'hl-from': String(at.from), 'hl-to': String(at.to) })
  }

  // ---------------------------------------------------------------- find
  let findOpen = $state(false)
  let query = $state('')
  let hits = $state<{ page: number; from: number; to: number }[]>([])
  let hitIndex = $state(0)
  let searching = $state(false)
  let findInput = $state<HTMLInputElement | null>(null)
  let searchRun = 0

  async function runFind() {
    const run = ++searchRun
    const q = query.trim().toLowerCase()
    hits = []
    hitIndex = 0
    if (!q || !doc) {
      searching = false
      return
    }
    searching = true
    const found: typeof hits = []
    for (let n = 1; n <= numPages; n++) {
      const pt = await textOf(n)
      if (run !== searchRun) return
      if (!pt) continue
      const flat = pt.text.replace(/\n/g, ' ')
      const lower = flat.toLowerCase()
      const hay = lower.length === flat.length ? lower : flat
      for (let i = hay.indexOf(q); i >= 0; i = hay.indexOf(q, i + q.length))
        found.push({ page: n, from: i, to: i + q.length })
      if (n % 20 === 0) {
        hits = [...found]
        await new Promise((r) => setTimeout(r, 0))
      }
    }
    if (run !== searchRun) return
    hits = found
    searching = false
    if (found.length) void showHit(0)
  }

  async function showHit(i: number) {
    if (hits.length === 0) return
    hitIndex = ((i % hits.length) + hits.length) % hits.length
    const hit = hits[hitIndex]
    await reveal(hit.page, `find:${hitIndex}`)
  }

  function openFind() {
    findOpen = true
    void tick().then(() => findInput?.select())
  }

  // ---------------------------------------------------------------- marks drawn on the pages
  const marksByPage = $derived.by(() => {
    const m = new Map<number, Mark[]>()
    const add = (n: number, mark: Mark) => {
      const l = m.get(n)
      if (l) l.push(mark)
      else m.set(n, [mark])
    }
    for (const h of sc.highlights)
      add(h.page, { key: `hl:${h.uid ?? h.blockId}`, kind: 'highlight', h, from: 0, to: 0 })
    if (findOpen)
      hits.forEach((hit, i) => {
        // a long document can have thousands of hits: only those near the screen are drawn
        if (Math.abs(hit.page - current) > 3 && i !== hitIndex) return
        add(hit.page, {
          key: `find:${i}`,
          kind: i === hitIndex ? 'found' : 'find',
          h: null,
          from: hit.from,
          to: hit.to,
        })
      })
    return m
  })

  const refs: Record<number, PdfPage | undefined> = {}

  /** Scrolls to the mark `key` on page `n` once the page is drawn (it may have to be drawn first). */
  async function reveal(n: number, key: string) {
    goToPage(n)
    for (let i = 0; i < 60; i++) {
      await new Promise((r) => requestAnimationFrame(r))
      const top = refs[n]?.boxTop(key)
      if (top != null && scroller) {
        scroller.scrollTop = layout.tops[n - 1] + top - viewH / 3
        updateVisible()
        return
      }
    }
  }

  let flashing = $state('')
  async function revealHighlight(h: Highlight) {
    sc.selected = h.uid ?? ''
    flashing = h.uid ?? ''
    setTimeout(() => (flashing = ''), 1500)
    await reveal(h.page, `hl:${h.uid ?? h.blockId}`)
  }

  async function openAt() {
    if (target.highlight) {
      const h = sc.highlights.find((x) => x.uid === target.highlight)
      if (h) return void revealHighlight(h)
    }
    goToPage(target.page ?? 1)
  }

  // ---------------------------------------------------------------- selecting text to highlight
  let bar = $state<{ x: number; y: number } | null>(null)

  function onSelectionChange() {
    const sel = window.getSelection()
    if (!sel || sel.isCollapsed || sel.rangeCount === 0 || !scroller) {
      bar = null
      return
    }
    const range = sel.getRangeAt(0)
    const inside = scroller.contains(range.commonAncestorContainer)
    if (!inside || areaMode) {
      bar = null
      return
    }
    const r = range.getBoundingClientRect()
    if (r.width === 0 && r.height === 0) {
      bar = null
      return
    }
    bar = { x: Math.min(Math.max(r.left + r.width / 2, 120), window.innerWidth - 120), y: r.top }
  }
  onMount(() => {
    document.addEventListener('selectionchange', onSelectionChange)
    return () => document.removeEventListener('selectionchange', onSelectionChange)
  })

  interface Segment {
    n: number
    from: number
    to: number
    pt: PageText
  }

  /** The part of each drawn page that the selection covers, as offsets in the text of that page. */
  function selectedSegments(): Segment[] {
    const sel = window.getSelection()
    if (!sel || sel.rangeCount === 0 || !scroller) return []
    const range = sel.getRangeAt(0)
    const out: Segment[] = []
    for (const el of Array.from(scroller.querySelectorAll<HTMLElement>('[data-testid="pdf-page"]'))) {
      if (!range.intersectsNode(el)) continue
      const n = Number(el.dataset.n)
      const page = refs[n]
      const pt = page?.text()
      if (!page || !pt) continue
      const text = el.querySelector('.textLayer')!
      const a = text.contains(range.startContainer)
        ? page.offsetAt(range.startContainer, range.startOffset)
        : 0
      const b = text.contains(range.endContainer)
        ? page.offsetAt(range.endContainer, range.endOffset)
        : pt.text.length
      if (a == null || b == null) continue
      let from = Math.min(a, b)
      let to = Math.max(a, b)
      // the white space at the ends is not part of the quote
      while (from < to && /\s/.test(pt.text[from])) from++
      while (to > from && /\s/.test(pt.text[to - 1])) to--
      if (to > from) out.push({ n, from, to, pt })
    }
    return out
  }

  async function highlightSelection(color: Color) {
    const segs = selectedSegments()
    if (segs.length === 0) return
    try {
      const ids = await sc.newIds(segs.length)
      await sc.insert(
        segs.map((s, i) =>
          textHighlightBlock({
            page: s.n,
            color,
            from: s.from,
            to: s.to,
            quote: quoteOf(s.pt.text, s.from, s.to),
            context: contextOf(s.pt.text, s.from, s.to),
            uid: ids[i],
          }),
        ),
      )
      window.getSelection()?.removeAllRanges()
      bar = null
      sc.selected = ids[ids.length - 1]
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  async function copySelection() {
    const text = window.getSelection()?.toString() ?? ''
    await copyText(text)
    window.getSelection()?.removeAllRanges()
    bar = null
  }

  // ---------------------------------------------------------------- area highlights
  let areaMode = $state(false)

  async function onArea(n: number, rect: Rect) {
    if (!doc) return
    try {
      const image = await cropPage(n, rect)
      await sc.ensure()
      const saved = await backend.call('save_asset', {
        page: sc.side!.page,
        data: image,
        name: `${fileName.replace(/\.pdf$/i, '')}-p${n}.png`,
      })
      const href = /\]\(([^)]*)\)/.exec(saved.markdown)?.[1]
      if (!href) throw new Error('the picture could not be stored')
      const [uid] = await sc.newIds(1)
      await sc.insert([areaHighlightBlock({ page: n, color: pen, rect, image: href, uid })])
      sc.selected = uid
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  /** The area of a page as a PNG (base64), drawn at twice the size the page has on the screen, at most 4000 px. */
  async function cropPage(n: number, r: Rect): Promise<string> {
    const page = await doc!.getPage(n)
    const base = page.getViewport({ scale: 1 })
    const k = Math.min(
      2 * Math.max(1, window.devicePixelRatio || 1) * scale,
      4000 / (Math.max(r[2] * base.width, r[3] * base.height) || 1),
    )
    const full = page.getViewport({ scale: k })
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(r[2] * full.width))
    canvas.height = Math.max(1, Math.round(r[3] * full.height))
    const ctx = canvas.getContext('2d')!
    ctx.fillStyle = '#fff'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
    // draw the page shifted so the wanted area lands on the canvas: only that part is rasterized
    await page.render({
      canvasContext: ctx,
      viewport: full,
      transform: [1, 0, 0, 1, -r[0] * full.width, -r[1] * full.height],
    }).promise
    return canvas.toDataURL('image/png').split(',')[1]
  }

  // ---------------------------------------------------------------- the popup of a highlight
  let pen = $state<Color>('yellow') // the colour of the next area highlight
  let popup = $state<{ h: Highlight; x: number; y: number } | null>(null)

  function onPick(h: Highlight, at: { x: number; y: number }) {
    popup = {
      h,
      x: Math.min(at.x, window.innerWidth - 260),
      y: Math.min(at.y + 10, window.innerHeight - 160),
    }
    sc.selected = h.uid ?? ''
  }

  function comment(h: Highlight) {
    panelOpen = true
    popup = null
    void sc.addComment(h)
  }

  // ---------------------------------------------------------------- the panel
  let panelOpen = $state(typeof window === 'undefined' || window.innerWidth > 900)

  function openNotesPage() {
    if (!sc.side?.page) return
    const name = sc.side.page
    finishPdf()
    location.hash = pageHash(name).replace(/^#/, '')
  }

  // ---------------------------------------------------------------- keys
  function key(e: KeyboardEvent) {
    // a key typed into a comment belongs to the editor of the block (its Escape ends the editing, not the viewer)
    if ((e.target as Element | null)?.closest?.('.cm-editor')) return
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      if (popup) popup = null
      else if (bar) {
        window.getSelection()?.removeAllRanges()
        bar = null
      } else if (areaMode) areaMode = false
      else if (findOpen) findOpen = false
      else finishPdf()
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault()
      openFind()
    } else if ((e.ctrlKey || e.metaKey) && (e.key === '+' || e.key === '=')) {
      e.preventDefault()
      void zoomBy(1.25)
    } else if ((e.ctrlKey || e.metaKey) && e.key === '-') {
      e.preventDefault()
      void zoomBy(0.8)
    } else if (e.key === 'Enter' && findOpen && document.activeElement === findInput) {
      e.preventDefault()
      void showHit(hitIndex + (e.shiftKey ? -1 : 1))
    }
  }

  let dialog = $state<HTMLDivElement | null>(null)
  onMount(() => dialog?.focus())
  void CONTEXT_CHARS
</script>

<div class="overlay" role="presentation">
  <div
    class="dialog"
    role="dialog"
    aria-label={`PDF ${fileName}`}
    aria-modal="true"
    tabindex="-1"
    data-testid="pdf-viewer"
    bind:this={dialog}
    onkeydown={key}
  >
    <header>
      <h2 title={target.path}>{fileName}</h2>
      <div class="group" role="group" aria-label="Page">
        <button
          type="button"
          onclick={() => goToPage(current - 1)}
          disabled={current <= 1}
          data-testid="pdf-prev"
          aria-label="Previous page">‹</button
        >
        <label class="pagebox"
          ><span class="sr">Page</span><input
            type="number"
            min="1"
            max={numPages}
            value={current}
            data-testid="pdf-page-box"
            aria-label="Page"
            onchange={(e) => goToPage(Number((e.currentTarget as HTMLInputElement).value))}
          />
          / {numPages}</label
        >
        <button
          type="button"
          onclick={() => goToPage(current + 1)}
          disabled={current >= numPages}
          data-testid="pdf-next"
          aria-label="Next page">›</button
        >
      </div>
      <div class="group" role="group" aria-label="Zoom">
        <button type="button" onclick={() => zoomBy(0.8)} aria-label="Zoom out" data-testid="pdf-zoom-out"
          >−</button
        >
        <button
          type="button"
          class="pct"
          onclick={() => setZoom(1)}
          title="Fit the width"
          data-testid="pdf-zoom-fit">{Math.round(zoom * 100)} %</button
        >
        <button type="button" onclick={() => zoomBy(1.25)} aria-label="Zoom in" data-testid="pdf-zoom-in"
          >+</button
        >
      </div>
      <button
        type="button"
        class="tool"
        class:on={findOpen}
        onclick={() => (findOpen ? (findOpen = false) : openFind())}
        data-testid="pdf-find-toggle"
        title="Find in the document (Ctrl+F)">Find</button
      >
      <button
        type="button"
        class="tool"
        class:on={areaMode}
        onclick={() => (areaMode = !areaMode)}
        data-testid="pdf-area-toggle"
        aria-pressed={areaMode}
        title="Draw a rectangle to highlight an area">Area</button
      >
      {#if areaMode}
        <span class="colors" role="group" aria-label="Colour of the area">
          {#each COLORS as c (c)}
            <button
              type="button"
              class="dot"
              class:on={pen === c}
              style:background={COLOR_HEX[c]}
              onclick={() => (pen = c)}
              aria-label={c}
              data-testid="pdf-pen-{c}"
            ></button>
          {/each}
        </span>
      {/if}
      <span class="spacer"></span>
      <button
        type="button"
        class="tool"
        class:on={panelOpen}
        onclick={() => (panelOpen = !panelOpen)}
        data-testid="pdf-panel-toggle"
        aria-pressed={panelOpen}>Highlights{sc.highlights.length ? ` (${sc.highlights.length})` : ''}</button
      >
      <button type="button" class="primary" onclick={finishPdf} data-testid="pdf-close">Close</button>
    </header>
    {#if findOpen}
      <div class="findbar" data-testid="pdf-find">
        <input
          bind:this={findInput}
          bind:value={query}
          oninput={() => void runFind()}
          placeholder="Find in the document"
          aria-label="Find in the document"
          data-testid="pdf-find-input"
        />
        <span class="count" data-testid="pdf-find-count" aria-live="polite"
          >{#if searching}Searching…{:else if query.trim()}{hits.length
              ? `${hitIndex + 1} of ${hits.length}`
              : 'No match'}{/if}</span
        >
        <button
          type="button"
          onclick={() => showHit(hitIndex - 1)}
          disabled={!hits.length}
          aria-label="Previous match"
          data-testid="pdf-find-prev">↑</button
        >
        <button
          type="button"
          onclick={() => showHit(hitIndex + 1)}
          disabled={!hits.length}
          aria-label="Next match"
          data-testid="pdf-find-next">↓</button
        >
      </div>
    {/if}

    <div class="body">
      <div
        class="scroller"
        class:area={areaMode}
        bind:this={scroller}
        bind:clientWidth={viewW}
        bind:clientHeight={viewH}
        onscroll={onScroll}
        data-testid="pdf-scroller"
      >
        {#if status === 'loading'}
          <p class="state" role="status">Opening the PDF…</p>
        {:else if status === 'error'}
          <p class="state error" role="alert" data-testid="pdf-error">{message}</p>
        {:else if doc}
          <div class="content" style:height="{layout.total}px">
            {#each layout.tops as top, i (i)}
              {@const n = i + 1}
              <div class="slot" style:top="{top}px" style:height="{pts(n).h * scale}px" data-slot={n}>
                {#if n >= mountFrom && n <= mountTo}
                  <PdfPage
                    bind:this={refs[n]}
                    {doc}
                    {n}
                    {scale}
                    width={pts(n).w * scale}
                    height={pts(n).h * scale}
                    marks={marksByPage.get(n) ?? []}
                    flash={flashing}
                    {areaMode}
                    {onSize}
                    {onText}
                    {onPlaced}
                    {onPick}
                    {onArea}
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      {#if panelOpen}
        <AnnotationPanel
          {sc}
          place={(h) => `p. ${h.page}`}
          notFound={(h) => `Not found on page ${h.page}.`}
          hint="Select text in the PDF and choose a colour."
          onReveal={revealHighlight}
          onComment={comment}
          onNotes={openNotesPage}
        />
      {/if}
    </div>

    {#if bar}
      <SelectionBar x={bar.x} y={bar.y} onColor={highlightSelection} onCopy={copySelection} />
    {/if}

    {#if popup}
      {@const h = popup.h}
      <HighlightPopup
        {h}
        x={popup.x}
        y={popup.y}
        onColor={(c) => {
          sc.setColor(h, c)
          popup = null
        }}
        onComment={() => comment(h)}
        onRef={() => {
          void copyText(refOf(h))
          popup = null
        }}
        onQuote={() => {
          void copyText(quoteBlock(h))
          popup = null
        }}
        onDelete={() => {
          sc.remove(h)
          popup = null
        }}
      />
    {/if}

    {#if password}
      <div class="ask" role="alertdialog" aria-label="Password" data-testid="pdf-password">
        <form
          onsubmit={(e) => {
            e.preventDefault()
            password?.answer(passwordText)
          }}
        >
          <p>{password.wrong ? 'That password is not right.' : 'This PDF has a password.'}</p>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            type="password"
            bind:value={passwordText}
            autofocus
            aria-label="Password"
            data-testid="pdf-password-input"
          />
          <button type="submit">Open</button>
          <button
            type="button"
            class="plain"
            onclick={() => {
              password?.answer(null)
              finishPdf()
            }}>Cancel</button
          >
        </form>
      </div>
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
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    margin: 2vh 2vw;
    min-height: 0;
    min-width: 0;
    background: var(--panel);
    border-radius: 10px;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5em 0.8em;
    padding: 0.5em 0.9em;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 1.05em;
    max-width: 22em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .group {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  button,
  .pagebox input,
  .findbar input {
    font: inherit;
    font-size: 0.9em;
    color: inherit;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: transparent;
    padding: 0.25em 0.7em;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .pagebox {
    display: flex;
    align-items: center;
    gap: 0.3em;
    font-size: 0.9em;
  }
  .pagebox input {
    width: 4em;
    cursor: text;
    text-align: right;
  }
  .pct {
    min-width: 4.5em;
    font-variant-numeric: tabular-nums;
  }
  .tool.on {
    background: var(--chip);
    border-color: var(--accent);
    font-weight: 600;
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .plain {
    border-color: transparent;
  }
  .findbar {
    display: flex;
    align-items: center;
    gap: 0.6em;
    padding: 0.4em 0.9em;
    border-bottom: 1px solid var(--border);
  }
  .findbar input {
    flex: 1;
    max-width: 24em;
    cursor: text;
  }
  .count {
    color: var(--muted);
    font-size: 0.85em;
    margin: 0;
  }
  .body {
    display: flex;
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .scroller {
    flex: 1;
    min-width: 0;
    overflow: auto;
    background: #6b6f76;
    position: relative;
    /* we handle the pinch ourselves */
    touch-action: pan-x pan-y;
  }
  .scroller.area {
    cursor: crosshair;
  }
  .content {
    position: relative;
  }
  .slot {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    justify-content: center;
  }
  .state {
    margin: 2em;
    color: #fff;
    text-align: center;
  }
  .state.error {
    color: #ffb4a8;
  }
  .colors {
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .dot {
    width: 1.5em;
    height: 1.5em;
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
  }
  .dot.on {
    border-color: var(--text, #111);
    box-shadow: 0 0 0 2px var(--panel);
  }
  .ask {
    position: absolute;
    inset: 0;
    background: rgb(0 0 0 / 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 95;
  }
  .ask form {
    background: var(--panel);
    padding: 1em;
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    gap: 0.5em;
  }
  .ask input {
    font: inherit;
    padding: 0.3em 0.6em;
  }
  /* a finger needs bigger targets */
  @media (pointer: coarse) {
    button,
    .pagebox input {
      min-height: 44px;
      min-width: 44px;
    }
    .dot {
      width: 44px;
      height: 44px;
    }
  }
  @media (max-width: 900px) {
    h2 {
      max-width: 9em;
    }
  }
</style>
