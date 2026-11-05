import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { expect, test, type Page } from './fixtures'
import { novel, NOVEL } from './epub-fixture'
import { highlight, reads } from './epub-reader-helpers'
import { makePdf, REPORT } from './pdf-fixture'

// LV-1..LV-8: a `file:///` link to a PDF or an EPUB opens in the viewer, in place; the system's program is one modifier,
// one button or one setting away. The dev server runs with --allow-local-files, as the desktop app does.
test.use({ localFiles: true })

/** A folder outside the graph, with a space and Polish letters in its name, as in real life. */
function folder(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'notatnik-local-ząb '))
}
const uriOf = (p: string) =>
  'file://' +
  p
    .split('/')
    .map((s) => encodeURIComponent(s))
    .join('/')

type Backend = Parameters<Parameters<typeof test>[2]>[0]['backend']

const blocks = (page: Page) => page.getByRole('tree', { name: 'Blocks' })

/** Records what the app asks the system or the browser to open. */
const stubWindowOpen = (page: Page) =>
  page.addInitScript(() => {
    ;(window as unknown as { __opened: string[] }).__opened = []
    window.open = ((u: string) => {
      ;(window as unknown as { __opened: string[] }).__opened.push(String(u))
      return null
    }) as typeof window.open
  })
const opened = (page: Page) => page.evaluate(() => (window as unknown as { __opened: string[] }).__opened)

function putPdf(dir: string, name = 'Rozrachunki do luty 2024_0001.pdf'): string {
  const p = path.join(dir, name)
  fs.writeFileSync(p, makePdf(REPORT))
  return p
}

async function note(backend: Backend, page: Page, text: string) {
  backend.write('pages/reading.md', text)
  await backend.open('/page/reading')
}

test('a click on a file:/// link opens the viewer in place; nothing is copied or written (LV-1)', async ({
  backend,
  page,
}) => {
  const dir = folder()
  const pdf = putPdf(dir)
  const before = fs.readFileSync(pdf)
  await note(backend, page, `- [Rozrachunki do luty 2024_0001.pdf](${uriOf(pdf)})\n`)
  await blocks(page)
    .getByRole('link', { name: /Rozrachunki/ })
    .click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText(
    'powerhouse of the cell',
    { timeout: 15_000 },
  )
  await expect(page.getByTestId('pdf-viewer').locator('h2')).toHaveText('Rozrachunki do luty 2024_0001.pdf')
  await expect(page.getByTestId('remote-bar')).toContainText('On this computer')
  // no sidecar until a highlight; no copy in the graph; the file is as it was
  expect(backend.exists('pages/hls')).toBe(false)
  expect(backend.exists('assets')).toBe(false)
  expect(fs.readFileSync(pdf).equals(before)).toBe(true)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('pdf-viewer')).toBeHidden()
})

test('Ctrl+click, the middle button and the arrow open the system program, not the viewer; the arrow is reached by the keyboard (LV-2)', async ({
  backend,
  page,
}) => {
  await stubWindowOpen(page)
  const pdf = putPdf(folder())
  const href = uriOf(pdf)
  await note(
    backend,
    page,
    `- [the report](${href})\n- [a book](${uriOf(pdf.replace('.pdf', '.epub'))})\n- [a site](https://example.org/)\n`,
  )
  fs.writeFileSync(pdf.replace('.pdf', '.epub'), novel())
  const link = blocks(page).getByRole('link', { name: 'the report' })

  await link.click({ modifiers: ['Control'] })
  await expect.poll(async () => (await opened(page)).at(-1)).toBe(href)
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)

  await link.click({ button: 'middle' })
  await expect.poll(async () => (await opened(page)).length).toBe(2)
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)

  // the arrow: not visible until the link is hovered or either has the focus (the pointer is still over the link)
  await page.mouse.move(0, 0)
  const arrow = blocks(page).getByRole('button', { name: /the report in the system/ })
  await expect(arrow).toHaveCSS('opacity', '0')
  await link.hover()
  await expect(arrow).toHaveCSS('opacity', '1')
  await page.mouse.move(0, 0)
  await expect(arrow).toHaveCSS('opacity', '0')
  // by keyboard: Tab to the link, Tab to the arrow, Enter
  await link.focus()
  await page.keyboard.press('Tab')
  await expect(arrow).toBeFocused()
  await expect(arrow).toHaveCSS('opacity', '1')
  await page.keyboard.press('Enter')
  await expect.poll(async () => (await opened(page)).length).toBe(3)
  expect((await opened(page)).at(-1)).toBe(href)
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)

  // a link to an EPUB has the same, a link to anything else has no arrow
  await expect(blocks(page).getByRole('button', { name: /a book in the system/ })).toHaveCount(1)
  await expect(blocks(page).getByRole('button', { name: /a site/ })).toHaveCount(0)
})

test('the setting turns it round: a click opens the system program, Ctrl+click the viewer; it is kept (LV-2)', async ({
  backend,
  page,
}) => {
  await stubWindowOpen(page)
  const pdf = putPdf(folder())
  const href = uriOf(pdf)
  backend.write('pages/reading.md', `- [the report](${href})\n`)
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Linked files' }).click()
  await page.getByLabel("The system's program").check()
  await page.reload()
  await page.getByRole('tab', { name: 'Linked files' }).click()
  await expect(page.getByLabel("The system's program")).toBeChecked()

  await page.goto(page.url().replace(/#.*$/, '') + '#/page/reading')
  const link = blocks(page).getByRole('link', { name: 'the report' })
  await link.click()
  await expect.poll(async () => (await opened(page)).at(-1)).toBe(href)
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  await link.click({ modifiers: ['Control'] })
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await page.keyboard.press('Escape')
  // the arrow now offers the viewer
  await expect(blocks(page).getByRole('button', { name: /the report in the app's viewer/ })).toHaveCount(1)
})

test('a PDF in the graph has the same two ways, and the viewer has a System viewer button (LV-3)', async ({
  backend,
  page,
}) => {
  await stubWindowOpen(page)
  fs.mkdirSync(path.join(backend.dir, 'assets'), { recursive: true })
  fs.writeFileSync(path.join(backend.dir, 'assets/report.pdf'), makePdf(REPORT))
  await note(backend, page, '- [the report](../assets/report.pdf)\n')
  const link = blocks(page).getByRole('link', { name: 'the report' })
  await link.click({ modifiers: ['Control'] })
  await expect.poll(async () => (await opened(page)).at(-1)).toContain('/asset?path=assets%2Freport.pdf')
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  await link.click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await page.getByTestId('doc-system').click()
  await expect.poll(async () => (await opened(page)).length).toBe(2)
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
})

test('a table cell, and a link that is no PDF or EPUB (LV-2, LV-3)', async ({ backend, page }) => {
  await stubWindowOpen(page)
  const pdf = putPdf(folder())
  const doc = path.join(path.dirname(pdf), 'notes.txt')
  fs.writeFileSync(doc, 'x')
  await note(
    backend,
    page,
    `- | what | where |\n  | --- | --- |\n  | report | [in a cell](${uriOf(pdf)}) |\n- [a text](${uriOf(doc)})\n`,
  )
  await blocks(page).getByRole('link', { name: 'in a cell' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await page.keyboard.press('Escape')
  // another type: untouched, no arrow, the system opens it
  await expect(blocks(page).getByRole('button', { name: /a text/ })).toHaveCount(0)
  await blocks(page).getByRole('link', { name: 'a text' }).click()
  await expect.poll(async () => (await opened(page)).at(-1)).toBe(uriOf(doc))
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
})

test('an EPUB in place: highlight, comment, reference; the file moves, the link is fixed, the highlight stays (LV-1, LV-4)', async ({
  backend,
  page,
}) => {
  const dir = folder()
  const book = path.join(dir, 'The Storm.epub')
  fs.writeFileSync(book, novel())
  await note(backend, page, `- The [book](${uriOf(book)})\n`)
  await blocks(page).getByRole('link', { name: 'book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await highlight(page, NOVEL.s1)
  await expect
    .poll(() => (backend.exists('pages/hls/The Storm.md') ? backend.read('pages/hls/The Storm.md') : ''))
    .toContain('hl-cfi:')
  await page.getByTestId('pdf-item').first().getByTestId('pdf-add-comment').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('check this')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/hls/The Storm.md')).toMatch(/^ {2}- check this$/m)
  const side = backend.read('pages/hls/The Storm.md')
  expect(side).toContain(`file: ${uriOf(book)}`)
  const uid = /\^([a-z0-9]{6})\n {2}hl-cfi::/.exec(side)![1]
  await page.getByTestId('epub-close').click()

  // a reference: the chip opens the book at the highlight
  backend.write('journals/2026/2026-10-03.md', `- Remember ((${uid}))\n`)
  await backend.open('/page/2026-10-03')
  await page.getByTestId('pdf-ref-chip').click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await expect(page.getByTestId('epub-mark').filter({ hasText: 'dark and stormy' })).toHaveAttribute(
    'data-flash',
    'true',
  )
  await page.getByTestId('epub-close').click()

  // the file is moved and renamed; the link in the note is fixed
  const moved = path.join(folder(), 'Moved storm.epub')
  fs.renameSync(book, moved)
  backend.write('pages/reading.md', `- The [book](${uriOf(moved)})\n`)
  await backend.open('/page/reading')
  await page.reload() // the page is shown from the file again, not from what the app held
  await blocks(page).getByRole('link', { name: 'book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  await expect(page.getByTestId('epub-mark').filter({ hasText: 'dark and stormy' })).toHaveCount(1)
  // one sidecar, and its `file` follows the file
  expect(fs.readdirSync(path.join(backend.dir, 'pages/hls'))).toEqual(['The Storm.md'])
  await expect.poll(() => backend.read('pages/hls/The Storm.md')).toContain(`file: ${uriOf(moved)}`)
})

test('Keep a copy in the graph copies the file and the old link finds the same highlights (LV-6)', async ({
  backend,
  page,
}) => {
  const dir = folder()
  const book = path.join(dir, 'The Storm.epub')
  fs.writeFileSync(book, novel())
  await note(backend, page, `- The [book](${uriOf(book)})\n`)
  await blocks(page).getByRole('link', { name: 'book' }).click()
  await reads(page, NOVEL.s1)
  await highlight(page, NOVEL.s1)
  await expect
    .poll(() => (backend.exists('pages/hls/The Storm.md') ? backend.read('pages/hls/The Storm.md') : ''))
    .toContain('hl-cfi:')
  await page.getByTestId('doc-keep').click()
  const kept = await page.getByTestId('doc-kept').innerText()
  const rel = /assets\/\d{4}\/\d{2}\/[^\s]+\.epub/.exec(kept)![0]
  expect(backend.exists(rel)).toBe(true)
  await expect.poll(() => backend.read('pages/hls/The Storm.md')).toContain(`file: ${rel}`)
  await page.getByTestId('epub-close').click()
  await blocks(page).getByRole('link', { name: 'book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  expect(fs.readdirSync(path.join(backend.dir, 'pages/hls'))).toEqual(['The Storm.md'])
  // the original was not touched
  expect(fs.existsSync(book)).toBe(true)
})

test('a file that is gone says so, offers the system program and keeps the highlights (LV-7)', async ({
  backend,
  page,
}) => {
  await stubWindowOpen(page)
  const pdf = putPdf(folder())
  const href = uriOf(pdf)
  await note(backend, page, `- [the report](${href})\n`)
  fs.rmSync(pdf)
  await blocks(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByText(/not there/)).toBeVisible()
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  await page.getByRole('button', { name: "Open in the system's program" }).click()
  await expect.poll(async () => (await opened(page)).at(-1)).toBe(href)
})

test('a file that is only named like a PDF is refused, not shown (LV-5)', async ({ backend, page }) => {
  const dir = folder()
  const fake = path.join(dir, 'fake.pdf')
  fs.writeFileSync(fake, 'this is not a pdf')
  await note(backend, page, `- [fake](${uriOf(fake)})\n`)
  await blocks(page).getByRole('link', { name: 'fake' }).click()
  await expect(page.getByText(/not a real PDF/)).toBeVisible()
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
})

test.describe('where the viewer cannot read files', () => {
  test.use({ localFiles: false })
  test('the system opens the file and a line says why (LV-1)', async ({ backend, page }) => {
    await stubWindowOpen(page)
    const pdf = putPdf(folder())
    const href = uriOf(pdf)
    await note(backend, page, `- [the report](${href})\n`)
    await blocks(page).getByRole('link', { name: 'the report' }).click()
    await expect.poll(async () => (await opened(page)).at(-1)).toBe(href)
    await expect(page.getByText(/viewer for files of this computer is in the desktop app/)).toBeVisible()
    await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  })
})
