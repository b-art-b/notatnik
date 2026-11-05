import fs from 'node:fs'
import path from 'node:path'
import { expect, test as t, type Frame, type Page } from './fixtures'
import { fixedLayoutBook, hostile, longBook, makeEpub, NOVEL, novel, styledBook } from './epub-fixture'

// EA-1..EA-10: the EPUB reader, highlights kept as blocks of a sidecar page, the panel, references.

type Backend = Parameters<Parameters<typeof t>[2]>[0]['backend']

function putBook(backend: Backend, rel: string, bytes: Buffer) {
  const full = path.join(backend.dir, rel)
  fs.mkdirSync(path.dirname(full), { recursive: true })
  fs.writeFileSync(full, bytes)
}

/** Puts a book in the graph with a note that links it, opens the note and clicks the link. */
async function openBook(page: Page, backend: Backend, bytes = novel(), name = 'novel.epub') {
  putBook(backend, `assets/books/${name}`, bytes)
  backend.write('pages/reading-notes.md', `- Read [The book](../assets/books/${name}) today\n`)
  await backend.open('/page/reading-notes')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'The book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
}

/** The frames of the chapters that are loaded (the reader shows them in iframes). */
const chapterFrames = (page: Page): Frame[] => page.frames().filter((f) => f !== page.mainFrame())

/** The text of the chapters that are loaded. */
async function bookText(page: Page): Promise<string> {
  const texts = await Promise.all(
    chapterFrames(page).map((f) =>
      f
        .locator('body')
        .innerText()
        .catch(() => ''),
    ),
  )
  return texts.join('\n')
}

const reads = (page: Page, text: string) =>
  expect.poll(() => bookText(page), { timeout: 15_000 }).toContain(text)

t(
  'a link to an EPUB in the graph opens the reader with the first chapter (EA-1)',
  async ({ backend, page }) => {
    await openBook(page, backend)
    await reads(page, NOVEL.s1)
    await expect(page.getByTestId('epub-reader').locator('h2')).toHaveText('The Storm')
    // looking at a book makes no sidecar page
    expect(backend.exists('pages/hls/novel.md')).toBe(false)
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('epub-reader')).toBeHidden()
  },
)

t(
  'next and previous turn the page, the percent grows, the contents open a chapter (EA-1)',
  async ({ backend, page }) => {
    await openBook(page, backend)
    await reads(page, NOVEL.s1)
    const percentNow = async () =>
      Number((await page.getByTestId('epub-percent').innerText()).replace(/\D/g, ''))
    const before = await percentNow()
    // forward until the second chapter is on the screen
    for (let i = 0; i < 20 && !(await bookText(page)).includes(NOVEL.s2); i++) {
      await page.getByTestId('epub-next').click()
      await page.waitForTimeout(150)
    }
    await reads(page, NOVEL.s2)
    await expect.poll(percentNow).toBeGreaterThan(before)
    await page.getByTestId('epub-prev').click()
    await reads(page, NOVEL.s1)
    await page.waitForTimeout(600) // the reader ignores a jump while a page is still turning
    // the contents list the three chapters; a click opens one
    await page.getByTestId('epub-toc-toggle').click()
    await expect(page.getByTestId('epub-toc-item')).toHaveText(['Loomings', 'The Storm', 'Calm'])
    await page.getByTestId('epub-toc-item').nth(2).click()
    await reads(page, NOVEL.s3)
    await expect(page.getByTestId('epub-toc')).toBeHidden()
    await expect(page.getByTestId('epub-chapter')).toHaveText('Calm')
  },
)

t(
  'a book of 60 chapters opens at once and keeps only a few chapters loaded (EA-1)',
  async ({ backend, page }) => {
    const started = Date.now()
    await openBook(page, backend, longBook(60), 'long.epub')
    await reads(page, 'Text of chapter 1 begins here.')
    expect(Date.now() - started).toBeLessThan(4000)
    for (const f of [0.5, 0.98]) {
      await page.getByTestId('epub-progress').fill(String(f))
      await page.getByTestId('epub-progress').dispatchEvent('change')
      await expect.poll(() => bookText(page)).toMatch(/chapter (2[5-9]|3\d|5[5-9]|60)/)
      expect(chapterFrames(page).length).toBeLessThanOrEqual(3)
    }
  },
)

t(
  'a book cannot run a script or fetch from the web, and only web links leave the app (EA-2)',
  async ({ backend, page }) => {
    const leaks: string[] = []
    await page.route(/leak\.invalid/, (route) => {
      leaks.push(route.request().url())
      return route.abort()
    })
    await page.addInitScript(() => {
      ;(window as unknown as { __opened: string[] }).__opened = []
      window.open = ((u: string) => {
        ;(window as unknown as { __opened: string[] }).__opened.push(String(u))
        return null
      }) as typeof window.open
    })
    await openBook(page, backend, hostile(), 'hostile.epub')
    await reads(page, 'Safe text before the traps.')
    const frame = chapterFrames(page).find((f) => f.url().startsWith('blob:'))!
    await frame.locator('#p1').click()
    await frame.locator('#js').click()
    await page.waitForTimeout(500)
    expect(await page.evaluate(() => (window as unknown as { __pwned?: string }).__pwned)).toBeUndefined()
    expect(leaks).toEqual([])
    // a web link goes to the shell; an ftp link and a javascript link do not
    await frame.locator('#web').click()
    await frame.locator('#ftp').click()
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { __opened: string[] }).__opened))
      .toEqual(['https://example.org/page'])
  },
)

t(
  'the theme, the size and the layout are kept, and so is the place where the reader stopped (EA-1)',
  async ({ backend, page }) => {
    await openBook(page, backend, longBook(30), 'long.epub')
    await reads(page, 'Text of chapter 1 begins here.')
    await page.getByTestId('epub-settings-toggle').click()
    await page.getByTestId('epub-theme-dark').click()
    await page.getByTestId('epub-size-up').click()
    await page.getByTestId('epub-size-up').click()
    await expect(page.getByTestId('epub-size')).toHaveText('22')
    await page.getByTestId('epub-flow-scrolled').click()
    // the chapter takes the colours of the theme
    await expect
      .poll(async () => chapterFrames(page)[0]?.evaluate(() => getComputedStyle(document.body).color))
      .toBe('rgb(214, 214, 214)')
    await page.getByTestId('epub-settings-toggle').click()
    await page.getByTestId('epub-progress').fill('0.5')
    await page.getByTestId('epub-progress').dispatchEvent('change')
    await expect.poll(() => bookText(page)).toMatch(/chapter (1[4-9]|2\d)/)
    const stopped = await page.getByTestId('epub-percent').innerText()
    await page.waitForTimeout(700) // the place is written a moment after the page turns
    await page.getByTestId('epub-close').click()
    await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'The book' }).click()
    await expect(page.getByTestId('epub-reader')).toBeVisible()
    await expect.poll(() => bookText(page)).toMatch(/chapter (1[4-9]|2\d)/)
    await expect(page.getByTestId('epub-percent')).toHaveText(stopped)
    await page.getByTestId('epub-settings-toggle').click()
    await expect(page.getByTestId('epub-size')).toHaveText('22')
    await expect(page.getByTestId('epub-flow-scrolled')).toHaveClass(/on/)
    await expect(page.getByTestId('epub-theme-dark')).toHaveClass(/on/)
    // another book starts at its beginning
    await page.getByTestId('epub-close').click()
    putBook(backend, 'assets/books/other.epub', longBook(5))
    backend.write('pages/other-notes.md', '- [Other](../assets/books/other.epub)\n')
    await backend.open('/page/other-notes')
    await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Other' }).click()
    await reads(page, 'Text of chapter 1 begins here.')
  },
)

t('find marks the hits across chapters and steps through them (EA-1)', async ({ backend, page }) => {
  await openBook(page, backend)
  await reads(page, NOVEL.s1)
  await page.keyboard.press('Control+f')
  await expect(page.getByTestId('epub-find-input')).toBeFocused()
  await page.keyboard.type('sea')
  await expect(page.getByTestId('epub-find-count')).toContainText(/1 of [2-9]/, { timeout: 10_000 })
  const count = await page.getByTestId('epub-find-count').innerText()
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('epub-find-count')).not.toHaveText(count)
  await page.getByTestId('epub-find-input').fill('qqqzzz')
  await expect(page.getByTestId('epub-find-count')).toHaveText('No match', { timeout: 10_000 })
})

t('a fixed-layout book and a file that is not an EPUB show a message (EA-1)', async ({ backend, page }) => {
  await openBook(page, backend, fixedLayoutBook(), 'fixed.epub')
  await expect(page.getByTestId('epub-error')).toContainText('fixed layout')
  await page.getByTestId('epub-close').click()
  putBook(backend, 'assets/books/bad.epub', Buffer.from('just some text'))
  backend.write('pages/bad-notes.md', '- [Bad](../assets/books/bad.epub)\n')
  await backend.open('/page/bad-notes')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Bad' }).click()
  await expect(page.getByTestId('epub-error')).toBeVisible()
})

t(
  'the reader code is not loaded with the first page, only when a book is opened (EA-1)',
  async ({ backend, page }) => {
    const urls: string[] = []
    page.on('request', (r) => urls.push(r.url()))
    putBook(backend, 'assets/books/novel.epub', novel())
    backend.write('pages/reading-notes.md', '- Read [The book](../assets/books/novel.epub) today\n')
    await backend.open('/page/reading-notes')
    await expect(
      page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'The book' }),
    ).toBeVisible()
    const lib = /paginator|epubcfi|zip-|overlayer|\/view-/i
    expect(urls.filter((u) => lib.test(u))).toEqual([])
    await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'The book' }).click()
    await reads(page, NOVEL.s1)
    expect(urls.some((u) => /paginator/i.test(u))).toBe(true)
  },
)

// ------------------------------------------------------------------ E4: highlighting text

/** Selects the text from the start of `from` to the end of `to` in the chapter that holds it (as a drag would). */
async function select(page: Page, from: string, to = from) {
  for (const f of chapterFrames(page)) {
    const done = await f.evaluate(
      ([from, to]) => {
        const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
        const nodes: { node: Text; start: number }[] = []
        let all = ''
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          nodes.push({ node: n as Text, start: all.length })
          all += (n as Text).data
        }
        const a = all.indexOf(from)
        if (a < 0) return false
        const b = all.indexOf(to, a) + to.length
        const at = (off: number) => {
          for (let i = nodes.length - 1; i >= 0; i--)
            if (off >= nodes[i].start) return { node: nodes[i].node, offset: off - nodes[i].start }
          throw new Error('no node')
        }
        const s = at(a)
        const e = at(b)
        const r = document.createRange()
        r.setStart(s.node, s.offset)
        r.setEnd(e.node, Math.min(e.offset, e.node.data.length))
        const sel = window.getSelection()!
        sel.removeAllRanges()
        sel.addRange(r)
        return true
      },
      [from, to] as const,
    )
    if (done) return
  }
  throw new Error(`text not found in any chapter: ${from}`)
}

async function highlight(page: Page, from: string, to = from, color = 'yellow') {
  await select(page, from, to)
  await expect(page.getByTestId('pdf-selbar')).toBeVisible()
  await page.getByTestId(`pdf-color-${color}`).click()
}

const sidecar = (backend: Backend) => backend.read('pages/hls/novel.md')
const CFI = /^ {2}hl-cfi:: (epubcfi\([^)]*\))$/m

/** Clicks on the words `text` in the chapter, where a highlight is drawn. */
async function clickWords(page: Page, text: string) {
  for (const f of chapterFrames(page)) {
    const rect = await f.evaluate((text) => {
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const i = (n as Text).data.indexOf(text)
        if (i < 0) continue
        const r = document.createRange()
        r.setStart(n, i)
        r.setEnd(n, i + Math.min(text.length, 12))
        const b = r.getBoundingClientRect()
        return { x: b.left + b.width / 2, y: b.top + b.height / 2 }
      }
      return null
    }, text)
    if (!rect) continue
    const frame = (await (await f.frameElement()).boundingBox())!
    await page.mouse.click(frame.x + rect.x, frame.y + rect.y)
    return
  }
  throw new Error(`words not found: ${text}`)
}

t(
  'selecting a sentence and choosing yellow adds one block with a CFI that leads back to the words (EA-4)',
  async ({ backend, page }) => {
    await openBook(page, backend)
    await reads(page, NOVEL.s1)
    await highlight(page, NOVEL.s1)
    await expect
      .poll(() => (backend.exists('pages/hls/novel.md') ? sidecar(backend) : ''))
      .toContain('hl-color:: yellow')
    const text = sidecar(backend)
    expect(text).toMatch(
      /^---\nfile: assets\/books\/novel\.epub\nsha: [0-9a-f]{16}\nkind: annotations\narchived: true\n---/,
    )
    expect(text).toMatch(
      /^- It was a dark and stormy night, and the rain fell in torrents\. \^[a-z0-9]{6}\n {2}hl-cfi:: epubcfi\([^)]*\)\n {2}hl-chapter:: 1 · Loomings\n {2}hl-color:: yellow\n {2}hl-context:: /m,
    )
    expect(text).not.toMatch(/hl-page|hl-from|hl-to/)
    // the viewer drew it from the CFI: the words under it are the quote
    await expect(page.getByTestId('epub-mark')).toHaveCount(1)
    await expect(page.getByTestId('epub-mark')).toHaveText(NOVEL.s1)
    await expect(page.getByTestId('epub-mark')).toHaveAttribute('data-color', 'yellow')
    await expect(page.getByTestId('pdf-selbar')).toBeHidden()
    await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  },
)

t('a selection of a whole page of text is one short property (EA-4)', async ({ backend, page }) => {
  await openBook(page, backend)
  await reads(page, NOVEL.s1)
  await highlight(page, NOVEL.s1, 'while the crew held on.')
  await expect.poll(() => (backend.exists('pages/hls/novel.md') ? sidecar(backend) : '')).toMatch(CFI)
  const cfi = CFI.exec(sidecar(backend))![1]
  expect(cfi.length).toBeLessThan(80)
  await expect(page.getByTestId('epub-mark')).toContainText('rain fell in torrents')
  await expect(page.getByTestId('epub-mark')).toContainText('the crew held on.')
})

t(
  'the highlight stays on the same words when the text size, theme, layout and window change (EA-4)',
  async ({ backend, page }) => {
    await openBook(page, backend)
    await reads(page, NOVEL.s1)
    await highlight(page, NOVEL.s1, NOVEL.s1, 'blue')
    await expect(page.getByTestId('epub-mark')).toHaveText(NOVEL.s1)
    const same = async () => {
      await reads(page, NOVEL.s1)
      await expect(page.getByTestId('epub-mark')).toHaveCount(1, { timeout: 10_000 })
      await expect(page.getByTestId('epub-mark')).toHaveText(NOVEL.s1)
    }
    await page.getByTestId('epub-settings-toggle').click()
    for (let i = 0; i < 4; i++) await page.getByTestId('epub-size-up').click()
    await same()
    await page.getByTestId('epub-theme-sepia').click()
    await same()
    await page.getByTestId('epub-flow-scrolled').click()
    await same()
    await page.getByTestId('epub-flow-paginated').click()
    await same()
    await page.setViewportSize({ width: 800, height: 600 })
    await same()
    // and it is still the same one block in the file
    expect(sidecar(backend).match(/^- /gm)).toHaveLength(1)
  },
)

t(
  'changing the colour rewrites only that property; deleting removes the block and the drawing (EA-4)',
  async ({ backend, page }) => {
    await openBook(page, backend)
    await reads(page, NOVEL.s1)
    await highlight(page, NOVEL.s1)
    await expect(page.getByTestId('epub-mark')).toHaveCount(1)
    const before = sidecar(backend)
    await clickWords(page, 'dark and stormy')
    await expect(page.getByTestId('pdf-popup')).toBeVisible()
    await page.getByTestId('pdf-popup-green').click()
    await expect.poll(() => sidecar(backend)).toContain('hl-color:: green')
    expect(sidecar(backend)).toBe(before.replace('hl-color:: yellow', 'hl-color:: green'))
    await expect(page.getByTestId('epub-mark')).toHaveAttribute('data-color', 'green')
    await clickWords(page, 'dark and stormy')
    await page.getByTestId('pdf-popup-delete').click()
    await expect.poll(() => sidecar(backend)).not.toContain('stormy')
    await expect(page.getByTestId('epub-mark')).toHaveCount(0)
  },
)

t(
  'one undo removes a highlight just made; one made elsewhere appears without reopening (EA-4, EA-8)',
  async ({ backend, page }) => {
    await openBook(page, backend)
    await reads(page, NOVEL.s1)
    await highlight(page, NOVEL.s1)
    await expect(page.getByTestId('epub-mark')).toHaveCount(1)
    const made = sidecar(backend)
    await page.keyboard.press('Control+z')
    await expect(page.getByTestId('epub-mark')).toHaveCount(0)
    backend.write('pages/hls/novel.md', made) // another device wrote the file
    await expect(page.getByTestId('epub-mark')).toHaveCount(1, { timeout: 10_000 })
  },
)

t(
  'a wrong CFI is repaired from the quote; a quote that is not in the book is listed as not found (EA-5)',
  async ({ backend, page }) => {
    await openBook(page, backend)
    await reads(page, NOVEL.s1)
    await highlight(page, NOVEL.s1)
    await expect.poll(() => (backend.exists('pages/hls/novel.md') ? sidecar(backend) : '')).toMatch(CFI)
    const good = sidecar(backend)
    const cfi = CFI.exec(good)![1]
    await page.getByTestId('epub-close').click()
    // the end of the range moved to the wrong place, and a second highlight that is not in the book at all
    const wrong = cfi.replace(/:(\d+)\)$/, ':7)')
    expect(wrong).not.toBe(cfi)
    backend.write(
      'pages/hls/novel.md',
      good.replace(cfi, wrong) +
        `- This sentence is nowhere in the book. ^zz9999\n  hl-cfi:: epubcfi(/6/2!/4/2/1,:0,:10)\n  hl-chapter:: 1 · Loomings\n  hl-color:: red\n  hl-context:: nothing | matches\n`,
    )
    await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'The book' }).click()
    await reads(page, NOVEL.s1)
    // the same words get a new CFI (written in another way than the one the reader made first)
    await expect.poll(() => sidecar(backend), { timeout: 10_000 }).not.toContain(wrong)
    expect(sidecar(backend)).toMatch(CFI)
    await expect(page.getByTestId('epub-mark')).toHaveCount(1)
    await expect(page.getByTestId('epub-mark')).toHaveText(NOVEL.s1)
    await expect(page.getByTestId('pdf-item-missing')).toHaveCount(1)
  },
)

// ------------------------------------------------------------------ E5: the panel

async function twoHighlights(page: Page, backend: Backend) {
  await openBook(page, backend)
  await reads(page, NOVEL.s1)
  await highlight(page, NOVEL.s1, NOVEL.s1, 'green')
  await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  await page.getByTestId('epub-toc-toggle').click()
  await page.getByTestId('epub-toc-item').nth(2).click()
  await reads(page, NOVEL.s3)
  await highlight(page, NOVEL.s3, NOVEL.s3, 'red')
  await expect(page.getByTestId('pdf-item')).toHaveCount(2)
}

t('the panel lists the highlights in reading order with their chapters (EA-6)', async ({ backend, page }) => {
  await twoHighlights(page, backend)
  const items = page.getByTestId('pdf-item')
  await expect(items.nth(0)).toContainText('1 · Loomings')
  await expect(items.nth(0)).toContainText('dark and stormy')
  await expect(items.nth(1)).toContainText('3 · Calm')
  await expect(page.getByTestId('pdf-count')).toHaveText('2 highlights')
})

t(
  'a comment typed in the panel is a child block; a TODO in it is on the agenda (EA-6)',
  async ({ backend, page }) => {
    await twoHighlights(page, backend)
    await page.getByTestId('pdf-item').first().getByTestId('pdf-add-comment').click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    await page.keyboard.type('TODO recheck this passage #reviewed')
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('epub-reader')).toBeVisible() // Escape ended the editing, not the reader
    await expect.poll(() => sidecar(backend)).toMatch(/^ {2}- TODO recheck this passage #reviewed$/m)
    expect(sidecar(backend)).toMatch(/dark and stormy[^\n]*\n( {2}hl-.*\n)+ {2}- TODO recheck/)
    expect(JSON.stringify(await backend.call('backlinks', { page: 'reviewed' }))).toContain('hls/novel')
    const agenda = JSON.stringify(await backend.call('agenda', { ns: null, tag: null, priority: null }))
    expect(agenda).toContain('recheck this passage')
    expect(agenda).toContain('dark and stormy')
  },
)

t(
  'a click on a quote opens the book at the highlight and marks it; the filters narrow the list (EA-6)',
  async ({ backend, page }) => {
    await twoHighlights(page, backend)
    await page.getByTestId('pdf-item').first().getByTestId('pdf-item-quote').click()
    await reads(page, NOVEL.s1)
    await expect(page.getByTestId('epub-mark').first()).toHaveAttribute('data-flash', 'true')
    await expect(page.getByTestId('pdf-item').first()).toHaveClass(/sel/)
    await page.getByTestId('pdf-filter-red').click()
    await expect(page.getByTestId('pdf-item')).toHaveCount(1)
    await expect(page.getByTestId('pdf-item')).toContainText('Calm')
    await page.getByTestId('pdf-filter-all').click()
    await page.getByTestId('pdf-filter-text').fill('lighthouse')
    await expect(page.getByTestId('pdf-item')).toHaveCount(0)
    await page.getByTestId('pdf-filter-text').fill('silent light')
    await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  },
)

t('a click on a highlight in the book selects its entry in the list (EA-6)', async ({ backend, page }) => {
  await twoHighlights(page, backend)
  await clickWords(page, 'Morning came')
  await expect(page.getByTestId('pdf-popup')).toBeVisible()
  await expect(page.getByTestId('pdf-item').nth(1)).toHaveClass(/sel/)
})

// ------------------------------------------------------------------ E6: references from the notes

t(
  'a reference to a highlight shows the quote and a chip that opens the book at the highlight (EA-7)',
  async ({ backend, page }) => {
    await twoHighlights(page, backend)
    const uid = /\^([a-z0-9]{6})\n {2}hl-cfi:: [^\n]*\n {2}hl-chapter:: 3/.exec(sidecar(backend))![1]
    await page.getByTestId('epub-close').click()
    backend.write('journals/2026/2026-10-03.md', `- Remember ((${uid})) for the exam\n`)
    await backend.open('/page/2026-10-03')
    await expect(page.getByRole('tree', { name: 'Blocks' }).locator('.block-ref')).toContainText(
      'Morning came',
    )
    const chip = page.getByTestId('pdf-ref-chip')
    await expect(chip).toHaveText('novel.epub · 3 · Calm')
    await chip.click()
    await expect(page.getByTestId('epub-reader')).toBeVisible()
    await reads(page, NOVEL.s3)
    await expect(page.getByTestId('epub-mark').filter({ hasText: 'Morning came' })).toHaveAttribute(
      'data-flash',
      'true',
    )
  },
)

t(
  'a link with #epubcfi(…) opens at that place; a bad one opens at the start; a removed highlight leaves a dead reference (EA-7)',
  async ({ backend, page }) => {
    await twoHighlights(page, backend)
    const cfi = /hl-cfi:: (epubcfi\([^)]*\))\n {2}hl-chapter:: 3/.exec(sidecar(backend))![1]
    const uid = /\^([a-z0-9]{6})\n {2}hl-cfi:: [^\n]*\n {2}hl-chapter:: 3/.exec(sidecar(backend))![1]
    await page.getByTestId('epub-close').click()
    backend.write(
      'pages/links.md',
      `- [place](<../assets/books/novel.epub#${cfi}>)\n- [nowhere](<../assets/books/novel.epub#epubcfi(/6/99!/4/2)>)\n- see ((${uid}))\n`,
    )
    await backend.open('/page/links')
    const tree = page.getByRole('tree', { name: 'Blocks' })
    await tree.getByRole('link', { name: 'place' }).click()
    await reads(page, NOVEL.s3)
    await page.getByTestId('epub-close').click()
    await tree.getByRole('link', { name: 'nowhere' }).click()
    await reads(page, NOVEL.s1) // the start of the book
    await page.getByTestId('epub-close').click()
    // the highlight is removed from the sidecar: the reference is dead and has no chip
    await expect(page.getByTestId('pdf-ref-chip')).toBeVisible()
    backend.write('pages/hls/novel.md', sidecar(backend).split('- Morning')[0])
    await expect(page.getByTestId('pdf-ref-chip')).toHaveCount(0, { timeout: 10_000 })
    await expect(tree.getByText('broken ref')).toBeVisible()
  },
)

t(
  'the style sheet of a book is applied and the policy refuses nothing it needs (EA-2)',
  async ({ backend, page }) => {
    const refused: string[] = []
    page.on('console', (m) => {
      if (/Content Security Policy|Refused to/i.test(m.text())) refused.push(m.text())
    })
    await openBook(page, backend, styledBook(), 'styled.epub')
    await reads(page, 'A styled paragraph of the book.')
    const frame = chapterFrames(page)[0]
    // the book's own rules reach the paragraph, also the one that came in through an @import
    await expect
      .poll(() => frame.locator('p.fancy').evaluate((p) => getComputedStyle(p).fontStyle))
      .toBe('italic')
    await expect
      .poll(() => frame.locator('p.fancy').evaluate((p) => getComputedStyle(p).textDecorationLine))
      .toBe('underline')
    expect(refused).toEqual([])
  },
)

// WA-5: a book that was clipped from a web page says where it came from.
const openedWindows = (page: Page) =>
  page.evaluate(() => (window as unknown as { __opened: string[] }).__opened)
const stubWindowOpen = (page: Page) =>
  page.addInitScript(() => {
    ;(window as unknown as { __opened: string[] }).__opened = []
    window.open = ((u: string) => {
      ;(window as unknown as { __opened: string[] }).__opened.push(String(u))
      return null
    }) as typeof window.open
  })
const ONE_PIXEL = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
)
const clipLike = (source?: string) =>
  makeEpub({
    title: 'A clipped page',
    source,
    files: [{ name: 'images/pic.png', type: 'image/png', bytes: ONE_PIXEL }],
    chapters: [
      {
        title: 'A clipped page',
        html:
          '<p>The text of the clipped page.</p><p><img src="images/pic.png" width="3000" height="2000" alt="wide"/></p>' +
          '<pre>' +
          'a very long line of code '.repeat(40) +
          '</pre>',
      },
    ],
  })

t(
  'a clip shows its host and Original page, which opens the source address (WA-5)',
  async ({ backend, page }) => {
    await stubWindowOpen(page)
    await openBook(page, backend, clipLike('https://www.example.org/news/a?b=1'), 'clip.epub')
    await expect(page.getByTestId('epub-host')).toHaveText('example.org')
    await page.getByTestId('epub-original').click()
    expect(await openedWindows(page)).toEqual(['https://www.example.org/news/a?b=1'])
  },
)

for (const [what, source] of [
  ['no source', undefined],
  ['a source that is not a web address', 'urn:isbn:9780000000002'],
  ['a javascript source', 'javascript:alert(1)'],
  ['a file source', 'file:///etc/passwd'],
] as const) {
  t(`a book with ${what} has no Original page button (WA-5)`, async ({ backend, page }) => {
    await openBook(page, backend, clipLike(source), 'clip.epub')
    await reads(page, 'The text of the clipped page.')
    await expect(page.getByTestId('epub-original')).toHaveCount(0)
    await expect(page.getByTestId('epub-host')).toHaveCount(0)
  })
}

t(
  'a clipped picture is shown, not wider than the page, in paginated and scrolled layout (WA-5)',
  async ({ backend, page }) => {
    await openBook(page, backend, clipLike('https://example.org/a'), 'clip.epub')
    await reads(page, 'The text of the clipped page.')
    const measure = async () => {
      const frame = chapterFrames(page).find((f) => f.url().startsWith('blob:'))!
      return frame.evaluate(() => {
        const img = document.querySelector('img')!
        return {
          loaded: img.complete,
          image: img.getBoundingClientRect().width,
          page: document.documentElement.clientWidth,
          pre: getComputedStyle(document.querySelector('pre')!).overflowX,
          preWidth: document.querySelector('pre')!.getBoundingClientRect().width,
        }
      })
    }
    for (const flow of ['paginated', 'scrolled']) {
      await page.getByTestId('epub-settings-toggle').click()
      await page.getByTestId(`epub-flow-${flow}`).click()
      await page.getByTestId('epub-settings-toggle').click()
      await expect
        .poll(async () => {
          const m = await measure().catch(() => null)
          return !!m && m.loaded && m.image > 0 && m.image <= m.page
        })
        .toBe(true)
      const m = await measure()
      expect(m.pre).toBe('auto')
      expect(m.preWidth).toBeLessThanOrEqual(m.page)
    }
  },
)
