<script lang="ts">
  // The EPUB reader (EA-1 to EA-8): a book of the graph in a full-screen view, with highlights and comments kept as
  // blocks of the book's sidecar page (`docs/phases/epub-annotation.md`). The EPUB itself is never written. foliate-js
  // does the reading and the layout (only `lib/epub/foliate.ts` talks to it); this component is the toolbar, the table
  // of contents, the search, the settings and the highlights.
  //
  // Svelte idioms used here: `$state` is a value the page re-draws from (`$state.raw` is one that is not wrapped,
  // for objects that are not ours, such as a DOM element); `$derived` is computed from other state; `$effect` runs after
  // the DOM exists and again when state it read changes; `untrack` reads without making an effect depend on it.
  import { onMount, untrack } from 'svelte'
  import { backend } from '../api'
  import { app, errorMessage, toast } from '../stores/app.svelte'
  import { finishBook } from '../commands/epub'
  import { pageHash } from '../route'
  import {
    BookError,
    foliate,
    openInto,
    titleOf,
    webSourceOf,
    hostOf,
    type FoliateView,
    type Location,
    type TocItem,
  } from '../epub/foliate'
  import { describeRange, docText, refind } from '../epub/docText'
  import { epubHighlightBlock, type Color, type Highlight } from '../pdf/highlights'
  import { COLOR_HEX } from '../annotate/colors'
  import { Sidecar, copyText, quoteBlock, refOf } from '../annotate/sidecar.svelte'
  import { isRemote } from '../annotate/remote'
  import RemoteBar from './RemoteBar.svelte'
  import type { RemoteOpened } from '../api'
  import AnnotationPanel from './AnnotationPanel.svelte'
  import HighlightPopup from './HighlightPopup.svelte'
  import SelectionBar from './SelectionBar.svelte'

  // `app.book` is set when the reader opens and stays the same until it closes: a snapshot is enough
  const target = untrack(() => app.book)!
  // a document of the graph's Nextcloud: `target.path` is its address, `info` what the backend made of it (NA-1)
  let info = $state<RemoteOpened | null>(null)
  const fileName = $derived(info?.name ?? target.path.split('/').pop() ?? target.path)
  const sc = new Sidecar(target.path)

  // ---------------------------------------------------------------- reading settings (per device, EA-1)
  type Theme = 'light' | 'sepia' | 'dark'
  type Flow = 'paginated' | 'scrolled'
  const THEMES: Record<Theme, { bg: string; fg: string; link: string }> = {
    light: { bg: '#ffffff', fg: '#1a1a1a', link: '#0b57d0' },
    sepia: { bg: '#f4ecd8', fg: '#5b4636', link: '#8a4b08' },
    dark: { bg: '#1e2126', fg: '#d6d6d6', link: '#8ab4f8' },
  }
  const PREFS_KEY = 'notatnik.epub.prefs'
  function loadPrefs(): { theme: Theme; size: number; flow: Flow } {
    const d = { theme: 'light' as Theme, size: 18, flow: 'paginated' as Flow }
    try {
      const p = JSON.parse(localStorage.getItem(PREFS_KEY) ?? '{}')
      return {
        theme: p.theme in THEMES ? p.theme : d.theme,
        size: Number.isFinite(p.size) ? Math.min(40, Math.max(10, p.size)) : d.size,
        flow: p.flow === 'scrolled' ? 'scrolled' : 'paginated',
      }
    } catch {
      return d
    }
  }
  const initial = loadPrefs()
  let theme = $state<Theme>(initial.theme)
  let size = $state(initial.size)
  let flow = $state<Flow>(initial.flow)
  let settingsOpen = $state(false)
  $effect(() => {
    const prefs = JSON.stringify({ theme, size, flow })
    try {
      localStorage.setItem(PREFS_KEY, prefs)
    } catch {
      // a private window may refuse; the settings then last until the reader closes
    }
  })

  const css = (t: Theme, px: number) => {
    const c = THEMES[t]
    return `
      html { background: ${c.bg} !important; color: ${c.fg} !important; font-size: ${px}px !important; }
      body, body * { color: ${c.fg} !important; background-color: transparent !important; }
      a:link, a:link *, a:visited, a:visited * { color: ${c.link} !important; }
      ::selection { background: rgb(0 90 255 / 0.3) !important; }
      /* a clipped page has wide pictures, tables and code: none is wider than the page, long ones scroll (WA-5) */
      img, svg, video { max-width: 100% !important; height: auto !important; }
      pre, table { max-width: 100% !important; overflow-x: auto !important; }
      table { display: block !important; }
    `
  }

  // ---------------------------------------------------------------- the book
  let host = $state<HTMLDivElement | null>(null)
  let view = $state.raw<FoliateView | null>(null)
  let status = $state<'loading' | 'ready' | 'error'>('loading')
  let message = $state('')
  let toc = $state<TocItem[]>([])
  let loc = $state<Location | null>(null)
  let tocOpen = $state(false)
  let sliderValue = $state(0)
  // the address of the web page a clip was made from (`dc:source`); books without one have no button
  const source = $derived(view ? webSourceOf(view.book) : null)
  const percent = $derived(Math.round((loc?.fraction ?? 0) * 100))

  const positionKey = () => `notatnik.epub.position.${sc.side?.sha ?? target.path}`
  let saveTimer = 0
  function rememberPosition(cfi: string) {
    window.clearTimeout(saveTimer)
    saveTimer = window.setTimeout(() => {
      try {
        localStorage.setItem(positionKey(), cfi)
      } catch {
        // as above
      }
    }, 400)
  }
  function savedPosition(): string | null {
    try {
      return localStorage.getItem(positionKey())
    } catch {
      return null
    }
  }

  onMount(() => {
    let dead = false
    void (async () => {
      try {
        let assetPath = target.path
        if (isRemote(target.path)) {
          const r = target.remote ?? (await backend.call('remote_open', { url: target.path }))
          if (!r.opened) throw new BookError('unreadable', 'This is not a document the app can open.')
          info = r
          assetPath = r.path
          sc.use(r.path, r.address)
        }
        const [res] = await Promise.all([fetch(backend.assetUrl(assetPath)).catch(() => null), sc.load()])
        if (!res || !res.ok) throw new BookError('unreadable', 'The file could not be read.')
        const v = await openInto(host!, fileName, await res.blob())
        if (dead) {
          closeLater(v)
          return
        }
        view = v
        toc = v.book.toc ?? []
        bind(v)
        v.renderer.setAttribute('flow', flow)
        v.renderer.setStyles(css(theme, size))
        status = 'ready'
        // where to open: the place of the link, a highlight, or where the reader stopped; one that leads nowhere
        // in this book (a bad CFI in a link) is dropped
        const wanted =
          target.cfi ?? sc.highlights.find((h) => h.uid === target.highlight)?.cfi ?? savedPosition()
        const first = wanted && leadsSomewhere(v, wanted) ? wanted : null
        await v.init({ lastLocation: first, showTextStart: !first })
        if (target.highlight) {
          const h = sc.highlights.find((x) => x.uid === target.highlight)
          if (h) void reveal(h)
        }
      } catch (e) {
        if (dead) return
        status = 'error'
        message = e instanceof BookError ? e.message : errorMessage(e)
      }
    })()
    return () => {
      dead = true
      window.clearTimeout(saveTimer)
      if (view) closeLater(view)
    }
  })

  /** foliate-js applies the styles on an animation frame; closing the view before that frame makes it fail on a view
   * that is gone. So the view is closed a moment after the reader. */
  function closeLater(v: FoliateView) {
    setTimeout(() => {
      try {
        v.close()
      } catch {
        // already gone
      }
      v.remove()
    }, 300)
  }

  function leadsSomewhere(v: FoliateView, cfi: string): boolean {
    try {
      const { index } = v.resolveCFI(cfi)
      return Number.isInteger(index) && index >= 0 && index < v.book.sections.length
    } catch {
      return false
    }
  }

  // settings that change the display
  $effect(() => {
    if (!view) return
    const [t, px, f] = [theme, size, flow]
    untrack(() => {
      view!.renderer.setAttribute('flow', f)
      view!.renderer.setStyles(css(t, px))
    })
  })

  // ---------------------------------------------------------------- events of the book
  /** The sections that are on the screen now, with their documents. */
  const contents = () => view?.renderer.getContents() ?? []

  function bind(v: FoliateView) {
    v.addEventListener('relocate', (e) => {
      const d = (e as CustomEvent<Location>).detail
      loc = d
      sliderValue = d.fraction
      if (d.cfi) rememberPosition(d.cfi)
    })
    v.addEventListener('load', (e) => {
      const { doc, index } = (e as CustomEvent<{ doc: Document; index: number }>).detail
      attach(doc, index)
      pruneMarks()
      refreshMarks()
    })
    v.addEventListener('create-overlay', () => drawAll())
    v.addEventListener('draw-annotation', (e) => {
      const { draw, annotation, range } = (
        e as CustomEvent<{
          draw: (
            f: (r: DOMRect[], o?: Record<string, unknown>) => SVGElement,
            o?: Record<string, unknown>,
          ) => void
          annotation: { value: string }
          range: Range
        }>
      ).detail
      const h = byCfi.get(annotation.value)
      if (!h) return
      void foliate().then((lib) => {
        const flash = h.uid !== null && h.uid === flashing
        draw(
          (rects, options) => {
            const g = lib.highlight(rects, options)
            if (flash) {
              const a = document.createElementNS('http://www.w3.org/2000/svg', 'animate')
              a.setAttribute('attributeName', 'opacity')
              a.setAttribute('values', '1;0.2;1')
              a.setAttribute('dur', '0.5s')
              a.setAttribute('repeatCount', '3')
              g.append(a)
            }
            return g
          },
          { color: COLOR_HEX[h.color] },
        )
        marks[annotation.value] = {
          uid: h.uid ?? '',
          color: h.color,
          text: range.toString(),
          index: indexOfCfi(annotation.value),
          flash,
        }
      })
    })
    v.addEventListener('show-annotation', (e) => {
      const { value, range } = (e as CustomEvent<{ value: string; range: Range }>).detail
      const h = byCfi.get(value)
      if (!h) return
      const r = range.getBoundingClientRect()
      const f = frameOf(range.commonAncestorContainer.ownerDocument)
      popup = {
        h,
        x: Math.min(Math.max(f.left + r.left, 8), window.innerWidth - 260),
        y: Math.min(f.top + r.bottom + 8, window.innerHeight - 200),
      }
      sc.selected = h.uid ?? ''
      turnLock = true
      setTimeout(() => (turnLock = false), 50)
    })
    v.addEventListener('external-link', (e) => {
      // the book never opens a window by itself: only a web or mail address leaves the app, through the shell
      e.preventDefault()
      const href = (e as CustomEvent<{ href_: string }>).detail.href_
      if (/^(https?:|mailto:)/i.test(href)) backend.openExternal(href)
    })
  }

  const frameOf = (doc: Document | null) =>
    (doc?.defaultView?.frameElement as HTMLElement | null)?.getBoundingClientRect() ?? new DOMRect()

  // the highlights as they are drawn now (also a list a screen reader or a test can read)
  interface Mark {
    uid: string
    color: Color
    text: string
    index: number
    flash: boolean
  }
  let marks = $state<Record<string, Mark>>({})
  const markList = $derived(Object.entries(marks))
  const byCfi = new Map<string, Highlight>()
  const drawnCfis = new Set<string>()
  const bad = new Set<string>() // CFIs that do not fit the book and were not found again: not drawn
  const found = new Set<string>() // highlights (by uid) that were found again by their quote
  let flashing = $state('')

  const indexOfCfi = (cfi: string): number => {
    try {
      return view!.resolveCFI(cfi).index
    } catch {
      return -1
    }
  }

  function pruneMarks() {
    const alive = new Set(contents().map((c) => c.index))
    for (const k of Object.keys(marks)) if (!alive.has(marks[k].index)) delete marks[k]
  }

  const strip = (s: string) => s.replace(/\s+/g, '')

  /** Checks the highlights of the chapters on the screen against their text, repairs the CFIs that no longer fit, and
   * draws the rest (EA-5). */
  function refreshMarks() {
    if (!view) return
    for (const { doc, index } of contents()) {
      let dt: ReturnType<typeof docText> | null = null
      for (const h of sc.highlights) {
        if (h.uid && found.has(h.uid) && !bad.has(h.cfi)) continue
        let resolved: { index: number; anchor: (d: Document) => Range } | null = null
        try {
          resolved = view.resolveCFI(h.cfi)
        } catch {
          resolved = null
        }
        if (resolved && resolved.index !== index) continue // it belongs to another chapter
        if (resolved) {
          try {
            const r = resolved.anchor(doc)
            if (strip(r.toString()) === strip(h.quote)) {
              bad.delete(h.cfi)
              sc.mark(h, true)
              continue
            }
          } catch {
            // the path no longer leads anywhere in this chapter: look for the quote
          }
        }
        dt ??= docText(doc)
        const r = refind(dt, h)
        if (r) {
          const cfi = view.getCFI(index, r)
          bad.delete(h.cfi)
          sc.mark(h, true)
          if (h.uid) found.add(h.uid)
          if (cfi !== h.cfi) {
            bad.add(h.cfi) // not drawn at the old place; drawn at the new one when the file has it
            sc.correct(h, { 'hl-cfi': cfi })
          }
        } else if (resolved) {
          bad.add(h.cfi)
          sc.mark(h, false)
        } else if (!(h.uid && found.has(h.uid))) {
          sc.mark(h, false) // no chapter has been found for it yet
        }
      }
    }
    drawAll()
  }

  function drawAll() {
    if (!view) return
    byCfi.clear()
    const want = new Set<string>()
    for (const h of sc.highlights) {
      byCfi.set(h.cfi, h)
      if (!bad.has(h.cfi)) want.add(h.cfi)
    }
    for (const cfi of drawnCfis)
      if (!want.has(cfi)) {
        drawnCfis.delete(cfi)
        delete marks[cfi]
        void view.deleteAnnotation({ value: cfi }).catch(() => undefined)
      }
    for (const cfi of want) {
      drawnCfis.add(cfi)
      void view.addAnnotation({ value: cfi }).catch(() => undefined) // a chapter that is not loaded has nothing to draw on
    }
  }

  // the highlights of the sidecar changed (a colour, a deleted block, one made on another device)
  $effect(() => {
    void sc.highlights
    void view
    untrack(() => refreshMarks())
  })

  // ---------------------------------------------------------------- inside a chapter: selection, keys, taps
  let bar = $state<{ x: number; y: number } | null>(null)
  let selected: { doc: Document; index: number } | null = null
  let turnLock = false
  let popup = $state<{ h: Highlight; x: number; y: number } | null>(null)

  function attach(doc: Document, index: number) {
    doc.addEventListener('selectionchange', () => {
      const sel = doc.getSelection()
      if (!sel || sel.isCollapsed || sel.rangeCount === 0) {
        if (selected?.doc === doc) {
          selected = null
          bar = null
        }
        return
      }
      const r = sel.getRangeAt(0).getBoundingClientRect()
      const f = frameOf(doc)
      selected = { doc, index }
      bar = {
        x: Math.min(Math.max(f.left + r.left + r.width / 2, 120), window.innerWidth - 120),
        y: Math.max(f.top + r.top, 60),
      }
    })
    doc.addEventListener('keydown', (e) => keys(e))
    doc.addEventListener('click', (e) => {
      // a tap near the left or right edge turns the page; a link, a selection and a highlight keep their own click
      if (flow !== 'paginated' || (e.target as Element).closest?.('a')) return
      if (!doc.getSelection()?.isCollapsed) return
      const f = frameOf(doc)
      const h = host!.getBoundingClientRect()
      const at = (f.left + e.clientX - h.left) / h.width
      if (at > 0.2 && at < 0.8) return
      setTimeout(() => {
        if (turnLock || popup) return
        void (at <= 0.2 ? view!.goLeft() : view!.goRight())
      }, 0)
    })
    let t0: { x: number; y: number } | null = null
    doc.addEventListener('touchstart', (e) => {
      t0 = e.touches.length === 1 ? { x: e.touches[0].clientX, y: e.touches[0].clientY } : null
    })
    doc.addEventListener('touchend', (e) => {
      const t = e.changedTouches[0]
      if (!t0 || flow !== 'paginated' || !doc.getSelection()?.isCollapsed) return
      const dx = t.clientX - t0.x
      const dy = t.clientY - t0.y
      t0 = null
      if (Math.abs(dx) > 60 && Math.abs(dy) < 50) void (dx < 0 ? view!.goRight() : view!.goLeft())
    })
    void index
  }

  async function highlightSelection(color: Color) {
    const s = selected
    const v = view
    if (!s || !v) return
    const sel = s.doc.getSelection()
    if (!sel || sel.rangeCount === 0) return
    const range = sel.getRangeAt(0).cloneRange()
    const d = describeRange(docText(s.doc), range)
    if (!d) return
    try {
      const cfi = v.getCFI(s.index, range)
      const label = v.getProgressOf(s.index, range).tocItem?.label
      const [uid] = await sc.newIds(1)
      await sc.insert([
        epubHighlightBlock({
          cfi,
          chapter: `${s.index + 1} · ${label ?? 'Section'}`,
          color,
          quote: d.quote,
          context: d.context,
          uid,
        }),
      ])
      sel.removeAllRanges()
      bar = null
      sc.selected = uid
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  async function copySelection() {
    const text = selected?.doc.getSelection()?.toString() ?? ''
    await copyText(text)
    selected?.doc.getSelection()?.removeAllRanges()
    bar = null
  }

  // ---------------------------------------------------------------- the panel and the popup
  let panelOpen = $state(typeof window === 'undefined' || window.innerWidth > 900)

  async function reveal(h: Highlight) {
    if (!view) return
    sc.selected = h.uid ?? ''
    flashing = h.uid ?? ''
    try {
      await view.goTo(h.cfi)
    } catch {
      // not in the book: the list says so
    }
    refreshMarks() // draws it again with the flash
    setTimeout(() => {
      flashing = ''
      for (const k of Object.keys(marks)) marks[k].flash = false
    }, 1800)
  }

  function comment(h: Highlight) {
    panelOpen = true
    popup = null
    void sc.addComment(h)
  }

  function openNotesPage() {
    if (!sc.side?.page) return
    const name = sc.side.page
    finishBook()
    location.hash = pageHash(name).replace(/^#/, '')
  }

  // ---------------------------------------------------------------- find
  let findOpen = $state(false)
  let query = $state('')
  let hits = $state<{ cfi: string; label: string }[]>([])
  let hitIndex = $state(0)
  let searching = $state(false)
  let findInput = $state<HTMLInputElement | null>(null)
  let searchRun = 0

  async function runFind() {
    const run = ++searchRun
    const q = query.trim()
    view?.clearSearch()
    hits = []
    hitIndex = 0
    if (!q || !view) {
      searching = false
      return
    }
    searching = true
    const found: typeof hits = []
    try {
      for await (const r of view.search({ query: q })) {
        if (run !== searchRun) return
        if (r === 'done') break
        if ('subitems' in r) {
          found.push(...r.subitems.map((s) => ({ cfi: s.cfi, label: r.label })))
          hits = [...found]
          if (found.length === r.subitems.length) void showHit(0)
        }
      }
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
    if (run === searchRun) searching = false
  }

  async function showHit(i: number) {
    if (hits.length === 0) return
    hitIndex = ((i % hits.length) + hits.length) % hits.length
    await view?.goTo(hits[hitIndex].cfi)
  }

  function openFind() {
    findOpen = true
    setTimeout(() => findInput?.select(), 0)
  }

  // ---------------------------------------------------------------- the table of contents
  function goToc(item: TocItem) {
    tocOpen = false
    void view?.goTo(item.href)
  }

  // ---------------------------------------------------------------- keys
  function keys(e: KeyboardEvent) {
    if ((e.target as Element | null)?.closest?.('input, textarea, .cm-editor')) return
    if (e.key === 'ArrowRight' || e.key === 'PageDown' || (e.key === ' ' && !e.shiftKey)) {
      e.preventDefault()
      void view?.goRight()
    } else if (e.key === 'ArrowLeft' || e.key === 'PageUp' || (e.key === ' ' && e.shiftKey)) {
      e.preventDefault()
      void view?.goLeft()
    } else if (e.key === 'Escape') {
      escape(e)
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault()
      openFind()
    }
  }

  function escape(e: Event) {
    e.preventDefault()
    e.stopPropagation()
    if (popup) popup = null
    else if (bar) {
      selected?.doc.getSelection()?.removeAllRanges()
      bar = null
    } else if (settingsOpen) settingsOpen = false
    else if (tocOpen) tocOpen = false
    else if (findOpen) {
      findOpen = false
      view?.clearSearch()
    } else finishBook()
  }

  function dialogKey(e: KeyboardEvent) {
    // a key typed into a comment belongs to the editor of the block
    if ((e.target as Element | null)?.closest?.('.cm-editor')) return
    if (e.key === 'Enter' && findOpen && document.activeElement === findInput) {
      e.preventDefault()
      void showHit(hitIndex + (e.shiftKey ? -1 : 1))
      return
    }
    keys(e)
  }

  let dialog = $state<HTMLDivElement | null>(null)
  onMount(() => dialog?.focus())
</script>

<div class="overlay" role="presentation">
  <div
    class="dialog"
    role="dialog"
    aria-label={`Book ${fileName}`}
    aria-modal="true"
    tabindex="-1"
    data-testid="epub-reader"
    bind:this={dialog}
    onkeydown={dialogKey}
  >
    <header>
      <button
        type="button"
        class="tool"
        class:on={tocOpen}
        onclick={() => (tocOpen = !tocOpen)}
        aria-pressed={tocOpen}
        data-testid="epub-toc-toggle">Contents</button
      >
      <h2 title={target.path}>{view ? titleOf(view.book) || fileName : fileName}</h2>
      {#if source}
        <span class="host" data-testid="epub-host" title={source}>{hostOf(source)}</span>
        <button
          type="button"
          class="tool"
          onclick={() => backend.openExternal(source)}
          title="Open the page this was clipped from, in the browser"
          data-testid="epub-original">Original page</button
        >
      {/if}
      <div class="group" role="group" aria-label="Page">
        <button
          type="button"
          onclick={() => view?.goLeft()}
          aria-label="Previous page"
          data-testid="epub-prev">‹</button
        >
        <button type="button" onclick={() => view?.goRight()} aria-label="Next page" data-testid="epub-next"
          >›</button
        >
      </div>
      <span class="chapter" data-testid="epub-chapter">{loc?.tocItem?.label ?? ''}</span>
      <span class="spacer"></span>
      <button
        type="button"
        class="tool"
        class:on={findOpen}
        onclick={() => (findOpen ? (findOpen = false) : openFind())}
        data-testid="epub-find-toggle"
        title="Find in the book (Ctrl+F)">Find</button
      >
      <div class="settings">
        <button
          type="button"
          class="tool"
          class:on={settingsOpen}
          onclick={() => (settingsOpen = !settingsOpen)}
          aria-pressed={settingsOpen}
          aria-label="Reading settings"
          data-testid="epub-settings-toggle">Aa</button
        >
        {#if settingsOpen}
          <div class="menu" role="group" aria-label="Reading settings" data-testid="epub-settings">
            <div class="row" role="group" aria-label="Theme">
              {#each ['light', 'sepia', 'dark'] as const as t (t)}
                <button
                  type="button"
                  class:on={theme === t}
                  onclick={() => (theme = t)}
                  data-testid="epub-theme-{t}">{t}</button
                >
              {/each}
            </div>
            <div class="row" role="group" aria-label="Font size">
              <button
                type="button"
                onclick={() => (size = Math.max(10, size - 2))}
                aria-label="Smaller text"
                data-testid="epub-size-down">A−</button
              >
              <span class="size" data-testid="epub-size">{size}</span>
              <button
                type="button"
                onclick={() => (size = Math.min(40, size + 2))}
                aria-label="Larger text"
                data-testid="epub-size-up">A+</button
              >
            </div>
            <div class="row" role="group" aria-label="Layout">
              {#each ['paginated', 'scrolled'] as const as f (f)}
                <button
                  type="button"
                  class:on={flow === f}
                  onclick={() => (flow = f)}
                  data-testid="epub-flow-{f}">{f}</button
                >
              {/each}
            </div>
          </div>
        {/if}
      </div>
      <button
        type="button"
        class="tool"
        class:on={panelOpen}
        onclick={() => (panelOpen = !panelOpen)}
        aria-pressed={panelOpen}
        data-testid="epub-panel-toggle"
        >Highlights{sc.highlights.length ? ` (${sc.highlights.length})` : ''}</button
      >
      <button type="button" class="primary" onclick={finishBook} data-testid="epub-close">Close</button>
    </header>
    {#if info}<RemoteBar {info} />{/if}

    {#if findOpen}
      <div class="findbar" data-testid="epub-find">
        <input
          bind:this={findInput}
          bind:value={query}
          oninput={() => void runFind()}
          placeholder="Find in the book"
          aria-label="Find in the book"
          data-testid="epub-find-input"
        />
        <span class="count" data-testid="epub-find-count" aria-live="polite"
          >{#if searching && hits.length === 0}Searching…{:else if query.trim()}{hits.length
              ? `${hitIndex + 1} of ${hits.length}`
              : searching
                ? 'Searching…'
                : 'No match'}{/if}</span
        >
        <button
          type="button"
          onclick={() => showHit(hitIndex - 1)}
          disabled={!hits.length}
          aria-label="Previous match"
          data-testid="epub-find-prev">↑</button
        >
        <button
          type="button"
          onclick={() => showHit(hitIndex + 1)}
          disabled={!hits.length}
          aria-label="Next match"
          data-testid="epub-find-next">↓</button
        >
      </div>
    {/if}

    <div class="body">
      {#if tocOpen}
        <nav class="toc" aria-label="Contents" data-testid="epub-toc">
          {#snippet items(list: TocItem[], depth: number)}
            <ul style:--depth={depth}>
              {#each list as item, i (i)}
                <li>
                  <button type="button" onclick={() => goToc(item)} data-testid="epub-toc-item"
                    >{item.label}</button
                  >
                  {#if item.subitems?.length}{@render items(item.subitems, depth + 1)}{/if}
                </li>
              {/each}
            </ul>
          {/snippet}
          {#if toc.length}{@render items(toc, 0)}{:else}<p class="empty">
              This book has no table of contents.
            </p>{/if}
        </nav>
      {/if}

      <div class="page" style:background={THEMES[theme].bg} data-testid="epub-page">
        <div class="host" bind:this={host}></div>
        {#if status === 'loading'}<p class="state" role="status">Opening the book…</p>{/if}
        {#if status === 'error'}<p class="state error" role="alert" data-testid="epub-error">
            {message}
          </p>{/if}
      </div>

      {#if panelOpen}
        <AnnotationPanel
          {sc}
          place={(h) => h.chapter || 'Book'}
          notFound={(h) => `Not found${h.chapter ? ` in ${h.chapter}` : ' in the book'}.`}
          hint="Select text in the book and choose a colour."
          onReveal={reveal}
          onComment={comment}
          onNotes={openNotesPage}
        />
      {/if}
    </div>

    <footer>
      <input
        type="range"
        min="0"
        max="1"
        step="0.001"
        bind:value={sliderValue}
        onchange={() => void view?.goToFraction(sliderValue)}
        aria-label="Position in the book"
        data-testid="epub-progress"
      />
      <span class="percent" data-testid="epub-percent">{percent} %</span>
    </footer>

    <!-- the highlights that are drawn in the chapter now, as a list (the drawing itself is an image) -->
    <ul class="marks" aria-label="Highlights on this page" data-testid="epub-marks">
      {#each markList as [cfi, m] (cfi)}
        <li data-testid="epub-mark" data-uid={m.uid} data-color={m.color} data-flash={m.flash}>{m.text}</li>
      {/each}
    </ul>

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
  .host {
    color: var(--muted);
    font-size: 0.85em;
    max-width: 10em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h2 {
    margin: 0;
    font-size: 1.05em;
    max-width: 18em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chapter {
    color: var(--muted);
    font-size: 0.85em;
    max-width: 16em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .group {
    display: flex;
    gap: 2px;
  }
  button,
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
  .tool.on,
  .menu button.on {
    background: var(--chip);
    border-color: var(--accent);
    font-weight: 600;
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .settings {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.3);
  }
  .menu .row {
    display: flex;
    gap: 6px;
    align-items: center;
    justify-content: center;
  }
  .size {
    min-width: 2.5em;
    text-align: center;
    font-variant-numeric: tabular-nums;
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
  .toc {
    width: 18em;
    max-width: 60%;
    overflow: auto;
    border-right: 1px solid var(--border);
    padding: 0.5em;
    background: var(--panel);
  }
  .toc ul {
    list-style: none;
    margin: 0;
    padding-left: calc(var(--depth) * 0.9em);
  }
  .toc li > button {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    padding: 0.3em 0.4em;
  }
  .toc li > button:hover {
    background: var(--chip);
  }
  .empty {
    color: var(--muted);
    font-size: 0.9em;
  }
  .page {
    flex: 1;
    min-width: 0;
    position: relative;
  }
  .host {
    position: absolute;
    inset: 0;
  }
  .host :global(foliate-view) {
    display: block;
    width: 100%;
    height: 100%;
  }
  .state {
    position: absolute;
    inset: 0;
    margin: 0;
    padding: 2em;
    text-align: center;
    color: var(--muted);
  }
  .state.error {
    color: #c0392b;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 0.8em;
    padding: 0.3em 0.9em;
    border-top: 1px solid var(--border);
  }
  footer input {
    flex: 1;
  }
  .percent {
    min-width: 3.5em;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    font-size: 0.85em;
  }
  .marks {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  @media (pointer: coarse) {
    button,
    footer input {
      min-height: 44px;
      min-width: 44px;
    }
  }
  @media (max-width: 900px) {
    h2 {
      max-width: 9em;
    }
    .host {
      display: none;
    }
    .chapter {
      display: none;
    }
    .toc {
      position: absolute;
      left: 0;
      top: 0;
      bottom: 0;
      z-index: 20;
      box-shadow: 4px 0 14px rgb(0 0 0 / 0.3);
    }
  }
</style>
