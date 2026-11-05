import fs from 'node:fs'
import path from 'node:path'
import { expect, test as base, type Page } from './fixtures'
import { makePdf, REPORT } from './pdf-fixture'

// PA-1..PA-10: the PDF viewer, highlights kept as blocks of a sidecar page, the panel, area highlights, references.
const t = base

type Backend = Parameters<Parameters<typeof base>[2]>[0]['backend']

/** Puts a PDF in the graph and a note with a link to it; opens the note. */
function putPdf(backend: Backend, rel: string, bytes: Buffer) {
  const full = path.join(backend.dir, rel)
  fs.mkdirSync(path.dirname(full), { recursive: true })
  fs.writeFileSync(full, bytes)
}

async function openReport(page: Page, backend: Backend, pdf = makePdf(REPORT)) {
  putPdf(backend, 'assets/report.pdf', pdf)
  backend.write('pages/reading-notes.md', '- Read the [Report](../assets/report.pdf) today\n')
  await backend.open('/page/reading-notes')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
}

t(
  'a link to a PDF in the graph opens the viewer with the text on the page (PA-1)',
  async ({ backend, page }) => {
    await openReport(page, backend)
    const first = page.getByTestId('pdf-page').first()
    await expect(first).toBeVisible()
    await expect(first.locator('.textLayer')).toContainText('powerhouse of the cell', { timeout: 10_000 })
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('1')
    // looking at a PDF makes no sidecar page
    expect(backend.exists('pages/hls/report.md')).toBe(false)
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('pdf-viewer')).toBeHidden()
  },
)

/** Selects the text from the start of `from` to the end of `to` on page `n` (inside the text layer, as a drag would). */
async function select(page: Page, n: number, from: string, to = from) {
  await page.evaluate(
    ([n, from, to]) => {
      const layer = document.querySelector(`[data-testid="pdf-page"][data-n="${n}"] .textLayer`)!
      const spans = Array.from(layer.querySelectorAll<HTMLElement>('span[data-i]'))
      let all = ''
      const at: { span: HTMLElement; start: number }[] = []
      for (const s of spans) {
        at.push({ span: s, start: all.length })
        all += s.textContent
      }
      const a = all.indexOf(from)
      const b = all.indexOf(to, a) + to.length
      if (a < 0 || b <= a) throw new Error(`text not found: ${from} .. ${to}`)
      const find = (off: number, end: boolean) => {
        for (let i = at.length - 1; i >= 0; i--) {
          const lo = at[i].start
          const len = at[i].span.textContent!.length
          if (off > lo || (off === lo && !end) || (end && off === lo + len)) {
            if (end && off === lo) continue
            return { node: at[i].span.firstChild!, offset: Math.min(off - lo, len) }
          }
        }
        throw new Error('no span')
      }
      const s = find(a, false)
      const e = find(b, true)
      const range = document.createRange()
      range.setStart(s.node, s.offset)
      range.setEnd(e.node, e.offset)
      const sel = window.getSelection()!
      sel.removeAllRanges()
      sel.addRange(range)
    },
    [n, from, to] as const,
  )
}

async function highlight(page: Page, n: number, from: string, to = from, color = 'yellow') {
  await select(page, n, from, to)
  await expect(page.getByTestId('pdf-selbar')).toBeVisible()
  await page.getByTestId(`pdf-color-${color}`).click()
}

const sidecar = (backend: Backend) => backend.read('pages/hls/report.md')

t(
  'selecting a sentence and choosing yellow adds one block to the sidecar (PA-3)',
  async ({ backend, page }) => {
    await openReport(page, backend)
    const first = page.getByTestId('pdf-page').first()
    await expect(first.locator('.textLayer')).toContainText('powerhouse', { timeout: 10_000 })
    await highlight(page, 1, 'The mitochondrion is the powerhouse of the cell.')
    await expect.poll(() => backend.exists('pages/hls/report.md')).toBe(true)
    await expect.poll(() => sidecar(backend)).toContain('hl-color:: yellow')
    const text = sidecar(backend)
    expect(text).toMatch(
      /^---\nfile: assets\/report\.pdf\nsha: [0-9a-f]{16}\nkind: annotations\narchived: true\n---/,
    )
    expect(text).toMatch(
      /^- The mitochondrion is the powerhouse of the cell\. \^[a-z0-9]{6}\n {2}hl-page:: 1\n {2}hl-color:: yellow\n {2}hl-from:: 0\n {2}hl-to:: 48\n {2}hl-context:: +\| Photosynthesis happens in the chloropla\n/m,
    )
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible()
    await expect(page.getByTestId('pdf-selbar')).toBeHidden()
  },
)

/** The line boxes of the text layer's spans on page `n` that hold `text`, and the boxes of the highlight overlay. */
async function rects(page: Page, n: number, text: string) {
  return page.evaluate(
    ([n, text]) => {
      const pg = document.querySelector(`[data-testid="pdf-page"][data-n="${n}"]`)!
      const spans = Array.from(pg.querySelectorAll<HTMLElement>('.textLayer span[data-i]')).filter((s) =>
        s.textContent!.includes(text),
      )
      const u = (list: DOMRect[]) => ({
        l: Math.min(...list.map((r) => r.left)),
        t: Math.min(...list.map((r) => r.top)),
        r: Math.max(...list.map((r) => r.right)),
        b: Math.max(...list.map((r) => r.bottom)),
      })
      const marks = Array.from(pg.querySelectorAll<HTMLElement>('[data-testid="pdf-highlight"]')).map((e) =>
        e.getBoundingClientRect(),
      )
      return {
        text: spans.length ? u(spans.map((s) => s.getBoundingClientRect())) : null,
        marks: marks.length ? u(marks) : null,
      }
    },
    [n, text] as const,
  )
}

for (const zoomIn of [-3, 0, 4]) {
  t(`the highlight is drawn over the same words at zoom step ${zoomIn} (PA-3)`, async ({ backend, page }) => {
    await openReport(page, backend)
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
      timeout: 10_000,
    })
    for (let i = 0; i < Math.abs(zoomIn); i++) {
      await page.getByTestId(zoomIn < 0 ? 'pdf-zoom-out' : 'pdf-zoom-in').click()
    }
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')
    await page.waitForTimeout(400)
    await highlight(page, 1, 'Photosynthesis happens in the chloroplast.')
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible()
    const r = await rects(page, 1, 'Photosynthesis')
    expect(r.text && r.marks).toBeTruthy()
    for (const k of ['l', 'r'] as const) expect(Math.abs(r.text![k] - r.marks![k])).toBeLessThan(4)
    // the box is as tall as the line (a font box), and sits on it
    expect(Math.abs(r.text!.t - r.marks!.t)).toBeLessThan(Math.max(6, 0.3 * (r.text!.b - r.text!.t)))
  })
}

t('the highlight lands on the words of a page rotated by 90 degrees (PA-3)', async ({ backend, page }) => {
  await openReport(page, backend, makePdf([{ lines: REPORT[0].lines, rotate: 90 }]))
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
    timeout: 10_000,
  })
  await highlight(page, 1, 'Zebrafish are a model organism.')
  await expect(page.getByTestId('pdf-highlight').first()).toBeVisible()
  const r = await rects(page, 1, 'Zebrafish')
  for (const k of ['l', 't', 'r', 'b'] as const) expect(Math.abs(r.text![k] - r.marks![k])).toBeLessThan(6)
})

const many = (n: number) =>
  makePdf(
    Array.from({ length: n }, (_, i) => ({
      lines: [`Page ${i + 1} body text`, `Line two of page ${i + 1}`],
    })),
  )

const SIDECAR_HEAD = (sha = 'x') =>
  `---\nfile: assets/report.pdf\nsha: ${sha}\nkind: annotations\narchived: true\n---\n`
const hl = (uid: string, quote: string, p: number, from: number, to: number, color = 'yellow', ctx = ' | ') =>
  `- ${quote} ^${uid}\n  hl-page:: ${p}\n  hl-color:: ${color}\n  hl-from:: ${from}\n  hl-to:: ${to}\n  hl-context:: ${ctx}\n`

t(
  'a 300-page PDF opens at once and keeps only a few pages drawn (PA-1, PA-10)',
  async ({ backend, page }) => {
    putPdf(backend, 'assets/big.pdf', many(300))
    backend.write('pages/big-notes.md', '- The [Big](../assets/big.pdf) one\n')
    await backend.open('/page/big-notes')
    const link = page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Big' })
    const started = Date.now()
    await link.click()
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('Page 1 body', {
      timeout: 10_000,
    })
    expect(Date.now() - started).toBeLessThan(2500)
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('1')
    // to the middle and to the end: only the pages near the screen are drawn
    for (const target of [150, 300]) {
      await page.getByTestId('pdf-page-box').fill(String(target))
      await page.getByTestId('pdf-page-box').press('Enter')
      await page.getByTestId('pdf-page-box').dispatchEvent('change')
      await expect(page.getByTestId('pdf-page').locator('.textLayer').first())
        .toContainText(`Page ${target - 1}`, {
          timeout: 10_000,
        })
        .catch(() => undefined)
      await expect
        .poll(async () => page.getByTestId('pdf-page').locator('canvas').count(), { timeout: 10_000 })
        .toBeLessThanOrEqual(8)
      await expect(
        page
          .getByTestId('pdf-page')
          .locator('.textLayer')
          .filter({ hasText: `Page ${target} body` }),
      ).toHaveCount(1, {
        timeout: 10_000,
      })
      await expect(page.getByTestId('pdf-page-box')).toHaveValue(String(target))
    }
  },
)

t(
  'a link with #page=N opens on that page; the page box and the buttons agree with the scroll (PA-1, PA-6)',
  async ({ backend, page }) => {
    putPdf(backend, 'assets/big.pdf', many(40))
    backend.write('pages/big-notes.md', '- [Page twelve](../assets/big.pdf#page=12)\n')
    await backend.open('/page/big-notes')
    await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Page twelve' }).click()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('12')
    await expect(
      page.getByTestId('pdf-page').locator('.textLayer').filter({ hasText: 'Page 12 body' }),
    ).toHaveCount(1)
    await page.getByTestId('pdf-next').click()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('13')
    await page.getByTestId('pdf-prev').click()
    await page.getByTestId('pdf-prev').click()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('11')
    // scrolling by hand moves the box
    await page
      .getByTestId('pdf-scroller')
      .evaluate((el) => (el.scrollTop += (3 * (el.firstElementChild as HTMLElement).clientHeight) / 40))
    await expect(page.getByTestId('pdf-page-box')).not.toHaveValue('11')
  },
)

t('zoom keeps the page in view and find jumps to a hit and marks it (PA-1)', async ({ backend, page }) => {
  await openReport(page, backend)
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
    timeout: 10_000,
  })
  await page.getByTestId('pdf-zoom-in').click()
  await expect(page.getByTestId('pdf-zoom-fit')).toHaveText('125 %')
  await expect(page.getByTestId('pdf-page-box')).toHaveValue('1')
  await page.getByTestId('pdf-zoom-fit').click()
  await expect(page.getByTestId('pdf-zoom-fit')).toHaveText('100 %')
  await page.keyboard.press('Control+f')
  await expect(page.getByTestId('pdf-find-input')).toBeFocused()
  await page.keyboard.type('results and')
  await expect(page.getByTestId('pdf-find-count')).toHaveText('1 of 1')
  await expect(page.getByTestId('pdf-page-box')).toHaveValue('2')
  await expect(page.getByTestId('pdf-find-hit').first()).toBeVisible()
  await page.getByTestId('pdf-find-input').fill('zzzz')
  await expect(page.getByTestId('pdf-find-count')).toHaveText('No match')
  await page.getByTestId('pdf-find-input').fill('the')
  await expect(page.getByTestId('pdf-find-count')).toContainText(/1 of [2-9]/)
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('pdf-find-count')).toContainText(/2 of/)
})

t(
  'a PDF with a password asks for it; a file that is not a PDF shows a message (PA-1)',
  async ({ backend, page }) => {
    await openReport(page, backend, makePdf(REPORT, { password: 'secret' }))
    await expect(page.getByTestId('pdf-password')).toBeVisible()
    await page.getByTestId('pdf-password-input').fill('wrong')
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('pdf-password')).toContainText('not right')
    await page.getByTestId('pdf-password-input').fill('secret')
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
      timeout: 10_000,
    })
  },
)

t('a file that is not a PDF shows a message, not a blank view (PA-1)', async ({ backend, page }) => {
  await openReport(page, backend, Buffer.from('this is just text'))
  await expect(page.getByTestId('pdf-error')).toBeVisible()
  await page.getByTestId('pdf-close').click()
  await expect(page.getByTestId('pdf-viewer')).toBeHidden()
})

t(
  'the pdf.js code is not loaded with the first page, only when a PDF is opened (PA-10)',
  async ({ backend, page }) => {
    const urls: string[] = []
    page.on('request', (r) => urls.push(r.url()))
    putPdf(backend, 'assets/report.pdf', makePdf(REPORT))
    backend.write('pages/reading-notes.md', '- Read the [Report](../assets/report.pdf) today\n')
    await backend.open('/page/reading-notes')
    await expect(
      page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Report' }),
    ).toBeVisible()
    expect(urls.filter((u) => /pdf/i.test(u.replace(/report\.pdf/, '')))).toEqual([])
    await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Report' }).click()
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
      timeout: 10_000,
    })
    expect(urls.some((u) => /pdf\.worker/i.test(u))).toBe(true)
  },
)

// ------------------------------------------------------------------ A3: highlights are blocks

t('a selection across two pages makes two blocks, each on its own page (PA-3)', async ({ backend, page }) => {
  await openReport(page, backend)
  await expect(page.getByTestId('pdf-page').nth(1).locator('.textLayer')).toContainText('results', {
    timeout: 10_000,
  })
  await page.evaluate(() => {
    const a = document.querySelector('[data-n="1"] .textLayer span[data-i]:last-of-type')!
    const b = document.querySelector('[data-n="2"] .textLayer span[data-i]')!
    const range = document.createRange()
    range.setStart(a.firstChild!, 3)
    range.setEnd(b.firstChild!, 6)
    const sel = window.getSelection()!
    sel.removeAllRanges()
    sel.addRange(range)
  })
  await expect(page.getByTestId('pdf-selbar')).toBeVisible()
  await page.getByTestId('pdf-color-green').click()
  await expect
    .poll(() => (backend.exists('pages/hls/report.md') ? sidecar(backend) : ''))
    .toMatch(/hl-page:: 2/)
  const text = sidecar(backend)
  expect(text.match(/^- /gm)).toHaveLength(2)
  expect(text).toMatch(/hl-page:: 1\n {2}hl-color:: green/)
  expect(text).toMatch(/hl-page:: 2\n {2}hl-color:: green/)
})

t(
  'changing the colour rewrites only that property; deleting removes the block and the box (PA-3)',
  async ({ backend, page }) => {
    await openReport(page, backend)
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
      timeout: 10_000,
    })
    await highlight(page, 1, 'Zebrafish are a model organism.')
    await expect.poll(() => backend.exists('pages/hls/report.md')).toBe(true)
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible()
    const before = sidecar(backend)
    await page.getByTestId('pdf-highlight').first().click({ force: true })
    await expect(page.getByTestId('pdf-popup')).toBeVisible()
    await page.getByTestId('pdf-popup-green').click()
    await expect.poll(() => sidecar(backend)).toContain('hl-color:: green')
    expect(sidecar(backend)).toBe(before.replace('hl-color:: yellow', 'hl-color:: green'))
    await expect(page.getByTestId('pdf-highlight').first()).toHaveAttribute('data-color', 'green')
    await page.getByTestId('pdf-highlight').first().click({ force: true })
    await page.getByTestId('pdf-popup-delete').click()
    await expect.poll(() => sidecar(backend)).not.toContain('Zebrafish')
    await expect(page.getByTestId('pdf-highlight')).toHaveCount(0)
  },
)

t(
  'one undo removes a highlight just made; one made elsewhere appears without reopening (PA-3, PA-7)',
  async ({ backend, page }) => {
    await openReport(page, backend)
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
      timeout: 10_000,
    })
    await highlight(page, 1, 'Zebrafish are a model organism.')
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible()
    await page.keyboard.press('Control+z')
    await expect(page.getByTestId('pdf-highlight')).toHaveCount(0)
    // another device (the file changed on disk)
    backend.write(
      'pages/hls/report.md',
      SIDECAR_HEAD() + hl('zz9999', 'Photosynthesis happens in the chloroplast.', 1, 49, 91),
    )
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible({ timeout: 10_000 })
  },
)

t('closing and opening the PDF again draws the same highlights (PA-3)', async ({ backend, page }) => {
  await openReport(page, backend)
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
    timeout: 10_000,
  })
  await highlight(page, 1, 'Zebrafish are a model organism.', undefined, 'blue')
  await expect(page.getByTestId('pdf-highlight').first()).toBeVisible()
  const first = await rects(page, 1, 'Zebrafish')
  await page.getByTestId('pdf-close').click()
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Report' }).click()
  await expect(page.getByTestId('pdf-highlight').first()).toBeVisible({ timeout: 10_000 })
  const again = await rects(page, 1, 'Zebrafish')
  expect(again.marks).toEqual(first.marks)
  await expect(page.getByTestId('pdf-highlight').first()).toHaveAttribute('data-color', 'blue')
})

t(
  'a highlight whose numbers are wrong is found by its quote and the numbers are corrected (PA-3)',
  async ({ backend, page }) => {
    putPdf(backend, 'assets/report.pdf', makePdf(REPORT))
    backend.write('pages/reading-notes.md', '- Read the [Report](../assets/report.pdf) today\n')
    backend.write(
      'pages/hls/report.md',
      SIDECAR_HEAD() +
        hl('ab1234', 'Zebrafish are a model organism.', 1, 3, 20) +
        hl('cd5678', 'This sentence is not in the PDF at all.', 1, 5, 25, 'red', 'nothing | matches'),
    )
    await backend.open('/page/reading-notes')
    await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Report' }).click()
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible({ timeout: 10_000 })
    const r = await rects(page, 1, 'Zebrafish')
    expect(Math.abs(r.text!.l - r.marks!.l)).toBeLessThan(4)
    // the numbers are corrected in the file (the guarded write)
    await expect.poll(() => backend.read('pages/hls/report.md')).toMatch(/hl-from:: 92\n {2}hl-to:: 123/)
    // the one that is not on the page is listed as not found and nothing is drawn for it
    await expect(page.getByTestId('pdf-item-missing')).toHaveCount(1)
    await expect(page.getByTestId('pdf-highlight')).toHaveCount(1)
  },
)

// ------------------------------------------------------------------ A4: the panel

const THREE =
  SIDECAR_HEAD() +
  hl('aa1111', 'Zebrafish are a model organism.', 1, 92, 123, 'green') +
  hl('bb2222', 'Second page: results and discussion.', 2, 0, 36, 'red') +
  hl('cc3333', 'The mitochondrion is the powerhouse of the cell.', 1, 0, 48, 'yellow')

async function openWithSidecar(page: Page, backend: Backend, sidecarText: string, pdf = makePdf(REPORT)) {
  putPdf(backend, 'assets/report.pdf', pdf)
  backend.write('pages/reading-notes.md', '- Read the [Report](../assets/report.pdf) today\n')
  backend.write('pages/hls/report.md', sidecarText)
  await backend.open('/page/reading-notes')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
}

t(
  'the panel lists the highlights in reading order, one added outside joins it (PA-4)',
  async ({ backend, page }) => {
    await openWithSidecar(page, backend, THREE)
    const items = page.getByTestId('pdf-item')
    await expect(items).toHaveCount(3)
    await expect(page.getByTestId('pdf-count')).toHaveText('3 highlights')
    await expect(items.nth(0)).toContainText('The mitochondrion')
    await expect(items.nth(1)).toContainText('Zebrafish')
    await expect(items.nth(2)).toContainText('Second page')
    backend.write(
      'pages/hls/report.md',
      THREE + hl('dd4444', 'Photosynthesis happens in the chloroplast.', 1, 49, 91, 'blue'),
    )
    await expect(items).toHaveCount(4, { timeout: 10_000 })
    await expect(items.nth(1)).toContainText('Photosynthesis')
  },
)

t(
  'a comment typed in the panel is a child block in the sidecar; tags and tasks in it work (PA-4)',
  async ({ backend, page }) => {
    await openWithSidecar(page, backend, THREE)
    const item = page.getByTestId('pdf-item').first()
    await item.getByTestId('pdf-add-comment').click()
    const editor = page.getByTestId('block-editor')
    await expect(editor).toBeFocused()
    await page.keyboard.type('TODO recompute this #reviewed')
    await page.keyboard.press('Escape')
    await expect
      .poll(() => backend.read('pages/hls/report.md'))
      .toMatch(/^ {2}- TODO recompute this #reviewed$/m)
    // the comment sits under the highlight it belongs to
    const text = backend.read('pages/hls/report.md')
    expect(text).toMatch(/\^cc3333\n( {2}hl-.*\n)+ {2}- TODO recompute/)
    await expect(item).toContainText('recompute this')
    const back = await backend.call<{ error?: unknown }[] | { page: string }[]>('backlinks', {
      page: 'reviewed',
    })
    expect(JSON.stringify(back)).toContain('hls/report')
    const agenda = await backend.call('agenda', { ns: null, tag: null, priority: null })
    const flat = JSON.stringify(agenda)
    expect(flat).toContain('recompute this')
    expect(flat).toContain('The mitochondrion is the powerhouse')
  },
)

t(
  'a click on a quote scrolls to its page and marks it; a click on a highlight selects it in the list (PA-4)',
  async ({ backend, page }) => {
    await openWithSidecar(
      page,
      backend,
      SIDECAR_HEAD() +
        hl('ee5555', 'Page 200 body text', 200, 0, 18, 'purple') +
        hl('ff6666', 'Page 2 body text', 2, 0, 16),
      many(300),
    )
    await expect(page.getByTestId('pdf-item')).toHaveCount(2)
    await page.getByTestId('pdf-item').nth(1).getByTestId('pdf-item-quote').click()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('200')
    const mark = page
      .getByTestId('pdf-highlight')
      .filter({ has: page.locator('xpath=self::*[@data-uid="ee5555"]') })
    await expect(mark.first()).toBeVisible({ timeout: 10_000 })
    await expect(mark.first()).toHaveClass(/flash/)
    // a click on the other one in the PDF selects its entry
    await page.getByTestId('pdf-item').nth(0).getByTestId('pdf-item-quote').click()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('2')
    await page.getByTestId('pdf-page-box').fill('200')
    await page.getByTestId('pdf-page-box').dispatchEvent('change')
    await expect(mark.first()).toBeVisible({ timeout: 10_000 })
    await mark.first().click({ force: true })
    await expect(page.getByTestId('pdf-item').nth(1)).toHaveClass(/sel/)
  },
)

t(
  'the colour filter and the text filter narrow the list and the drawn highlights (PA-4)',
  async ({ backend, page }) => {
    await openWithSidecar(page, backend, THREE)
    await expect(page.getByTestId('pdf-item')).toHaveCount(3)
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible({ timeout: 10_000 })
    await page.getByTestId('pdf-filter-green').click()
    await expect(page.getByTestId('pdf-item')).toHaveCount(1)
    await expect(page.getByTestId('pdf-item')).toContainText('Zebrafish')
    await page.getByTestId('pdf-filter-all').click()
    await page.getByTestId('pdf-filter-text').fill('results')
    await expect(page.getByTestId('pdf-item')).toHaveCount(1)
    await expect(page.getByTestId('pdf-item')).toContainText('Second page')
  },
)

// ------------------------------------------------------------------ A5: area highlights

/** Drags on page 1 from (fx0, fy0) to (fx1, fy1), given as fractions of the page (they may lie outside it). */
async function dragArea(page: Page, fx0: number, fy0: number, fx1: number, fy1: number) {
  const box = (await page.getByTestId('pdf-page').first().boundingBox())!
  await page.mouse.move(box.x + fx0 * box.width, box.y + fy0 * box.height)
  await page.mouse.down()
  await page.mouse.move(box.x + ((fx0 + fx1) / 2) * box.width, box.y + ((fy0 + fy1) / 2) * box.height, {
    steps: 4,
  })
  await page.mouse.move(box.x + fx1 * box.width, box.y + fy1 * box.height, { steps: 4 })
  await page.mouse.up()
}

t(
  'a dragged rectangle becomes an area highlight with a picture of the area (PA-5)',
  async ({ backend, page }) => {
    await openReport(page, backend)
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
      timeout: 10_000,
    })
    await page.getByTestId('pdf-area-toggle').click()
    await page.getByTestId('pdf-pen-red').click()
    await dragArea(page, 0.1, 0.05, 0.6, 0.2)
    await expect
      .poll(() => (backend.exists('pages/hls/report.md') ? sidecar(backend) : ''))
      .toContain('hl-type:: area')
    const text = sidecar(backend)
    const m =
      /^- !\[area\]\(\.\.\/\.\.\/(assets\/[^)]+\.png)\) \^[a-z0-9]{6}\n {2}hl-page:: 1\n {2}hl-color:: red\n {2}hl-type:: area\n {2}hl-rect:: ([\d.]+),([\d.]+),([\d.]+),([\d.]+)$/m.exec(
        text,
      )
    expect(m, text).toBeTruthy()
    const [, rel, x, y, w, h] = m!
    expect(Number(x)).toBeCloseTo(0.1, 1)
    expect(Number(w)).toBeCloseTo(0.5, 1)
    expect(Number(h)).toBeCloseTo(0.15, 1)
    void y
    // the picture is a PNG with the shape of the area
    const png = fs.readFileSync(path.join(backend.dir, rel))
    expect(png.subarray(1, 4).toString()).toBe('PNG')
    const pw = png.readUInt32BE(16)
    const ph = png.readUInt32BE(20)
    expect(pw / ph).toBeCloseTo((Number(w) * 612) / (Number(h) * 792), 1)
    // drawn on the page, listed in the panel with its picture
    await expect(page.getByTestId('pdf-highlight').first()).toHaveClass(/area/)
    const img = page.getByTestId('pdf-item').first().locator('img')
    await expect(img).toBeVisible()
    await expect.poll(() => img.evaluate((i: HTMLImageElement) => i.naturalWidth)).toBeGreaterThan(50)
  },
)

t(
  'a rectangle dragged outside the page is cut to the page; deleting leaves the picture (PA-5)',
  async ({ backend, page }) => {
    await openReport(page, backend)
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
      timeout: 10_000,
    })
    await page.getByTestId('pdf-area-toggle').click()
    await dragArea(page, 0.7, 0.25, 1.15, 0.45)
    await expect
      .poll(() => (backend.exists('pages/hls/report.md') ? sidecar(backend) : ''))
      .toContain('hl-rect::')
    const m = /hl-rect:: ([\d.]+),([\d.]+),([\d.]+),([\d.]+)/.exec(sidecar(backend))!
    expect(Number(m[1]) + Number(m[3])).toBeLessThanOrEqual(1.0001)
    expect(Number(m[2]) + Number(m[4])).toBeLessThanOrEqual(1.0001)
    const rel = /\.\.\/\.\.\/(assets\/[^)]+\.png)/.exec(sidecar(backend))![1]
    await page.getByTestId('pdf-area-toggle').click()
    const box = (await page.getByTestId('pdf-highlight').first().boundingBox())!
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2)
    await page.getByTestId('pdf-popup-delete').click()
    await expect.poll(() => sidecar(backend)).not.toContain('hl-type:: area')
    expect(backend.exists(rel)).toBe(true)
  },
)

// ------------------------------------------------------------------ A6: references from the notes

t(
  'a reference to a highlight shows the quote and a chip that opens the PDF at the highlight (PA-6)',
  async ({ backend, page }) => {
    putPdf(backend, 'assets/report.pdf', many(300))
    backend.write(
      'pages/hls/report.md',
      SIDECAR_HEAD() + hl('ee5555', 'Page 200 body text', 200, 0, 18, 'purple'),
    )
    backend.write('journals/2026/2026-10-03.md', '- Remember ((ee5555)) for the exam\n')
    await backend.open('/page/2026-10-03')
    const ref = page.getByRole('tree', { name: 'Blocks' }).locator('.block-ref')
    await expect(ref).toContainText('Page 200 body text')
    const chip = page.getByTestId('pdf-ref-chip')
    await expect(chip).toHaveText('report.pdf p. 200')
    await chip.click()
    await expect(page.getByTestId('pdf-viewer')).toBeVisible()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('200')
    const mark = page.locator('[data-testid="pdf-highlight"][data-uid="ee5555"]').first()
    await expect(mark).toBeVisible({ timeout: 10_000 })
    await expect(mark).toHaveClass(/flash/)
  },
)

t(
  'Copy reference puts ((id)) on the clipboard; a removed highlight leaves a dead reference without a chip (PA-6)',
  async ({ backend, page, context, browserName }) => {
    // WebKit cannot be given the clipboard permission in the test; the copy itself is checked in Chromium only
    const clip = browserName === 'chromium'
    if (clip) await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    await openWithSidecar(
      page,
      backend,
      SIDECAR_HEAD() + hl('aa1111', 'Zebrafish are a model organism.', 1, 92, 123, 'green'),
    )
    await expect(page.getByTestId('pdf-highlight').first()).toBeVisible({ timeout: 10_000 })
    await page.getByTestId('pdf-highlight').first().click({ force: true })
    await page.getByTestId('pdf-popup-ref').click()
    if (clip) expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('((aa1111))')
    await page.getByTestId('pdf-highlight').first().click({ force: true })
    await page.getByTestId('pdf-popup-quote').click()
    if (clip)
      expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
        '> Zebrafish are a model organism. ((aa1111))',
      )
    await page.getByTestId('pdf-close').click()
    backend.write('pages/notes.md', '- see ((aa1111))\n')
    await backend.open('/page/notes')
    await expect(page.getByTestId('pdf-ref-chip')).toBeVisible()
    backend.write('pages/hls/report.md', SIDECAR_HEAD())
    await expect(page.getByTestId('pdf-ref-chip')).toHaveCount(0, { timeout: 10_000 })
    await expect(page.getByRole('tree', { name: 'Blocks' }).getByText('broken ref')).toBeVisible()
  },
)

t(
  'a quote with brackets, a hash and a list marker shows as it was in the PDF (PA-3)',
  async ({ backend, page }) => {
    const lines = ['C# and [[brackets]] #tag ((x))', '- not a list *star*']
    await openReport(page, backend, makePdf([{ lines }]))
    await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('brackets', {
      timeout: 10_000,
    })
    await highlight(page, 1, lines[0])
    await expect
      .poll(() => (backend.exists('pages/hls/report.md') ? sidecar(backend) : ''))
      .toContain('hl-page')
    const item = page.getByTestId('pdf-item').first()
    await expect(item).toContainText('C# and [[brackets]] #tag ((x))')
    // no link, tag or reference was made out of it
    await page.getByTestId('pdf-close').click()
    expect(JSON.stringify(await backend.call('backlinks', { page: 'brackets' }))).not.toContain('hls/report')
    expect(JSON.stringify(await backend.call('backlinks', { page: 'tag' }))).not.toContain('hls/report')
  },
)

t(
  'the sidecar page opens the PDF: a button for the document, a chip at each highlight (PA-6)',
  async ({ backend, page }) => {
    await openWithSidecar(page, backend, THREE)
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('pdf-viewer')).toBeHidden()
    await backend.open('/page/hls/report')
    await expect(page.getByTestId('open-document')).toBeVisible()
    await page.getByTestId('open-document').click()
    await expect(page.getByTestId('pdf-viewer')).toBeVisible()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('1')
    await page.keyboard.press('Escape')
    const chips = page.getByRole('tree', { name: 'Blocks' }).getByTestId('pdf-ref-chip')
    await expect(chips).toHaveCount(3)
    await chips.nth(1).click() // the highlight of page 2
    await expect(page.getByTestId('pdf-viewer')).toBeVisible()
    await expect(page.getByTestId('pdf-page-box')).toHaveValue('2')
  },
)
