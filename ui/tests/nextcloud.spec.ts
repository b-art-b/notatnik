import fs from 'node:fs'
import path from 'node:path'
import { expect, type Page } from './fixtures'
import { makeEpub, NOVEL, novel } from './epub-fixture'
import { highlight, reads } from './epub-reader-helpers'
import { makePdf, REPORT } from './pdf-fixture'
import { account, fileId, put, synced, test } from './nextcloud-fixture'

// NA-1..NA-8: PDFs and EPUBs of the graph's Nextcloud open in the viewers through a cache of this device, against the
// in-memory WebDAV server of the sync tests (it numbers its files like Nextcloud and answers SEARCH by number).

/** Records what the app asks the browser to open. */
const stubWindowOpen = (page: Page) =>
  page.addInitScript(() => {
    ;(window as unknown as { __opened: string[] }).__opened = []
    window.open = ((u: string) => {
      ;(window as unknown as { __opened: string[] }).__opened.push(String(u))
      return null
    }) as typeof window.open
  })
const opened = (page: Page) => page.evaluate(() => (window as unknown as { __opened: string[] }).__opened)

const links = (page: Page) => page.getByRole('tree', { name: 'Blocks' })

test('a link to a PDF on the graph server opens the viewer with the text; nothing lands in the graph (NA-1, NA-2)', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Docs/Report.pdf', makePdf(REPORT))
  const g = await synced(backend, dav)
  const id = await fileId(dav, 'Docs/Report.pdf')
  g.write('pages/reading.md', `- Read [the report](${dav.server}/f/${id}) today\n`)
  await backend.open('/page/reading')
  await links(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText(
    'powerhouse of the cell',
    { timeout: 15_000 },
  )
  // the file's own name is the title, and the bar says where it is from
  await expect(page.getByTestId('pdf-viewer').locator('h2')).toHaveText('Report.pdf')
  await expect(page.getByTestId('remote-bar')).toContainText('Nextcloud')
  await expect(page.getByTestId('doc-offline')).toHaveCount(0)
  // looking at a document makes no sidecar and puts no file in the graph
  expect(g.exists('pages/hls')).toBe(false)
  expect(g.exists('assets')).toBe(false)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('pdf-viewer')).toBeHidden()
})

test('a link by path opens too; a file of another type, another server and a plain web link go to the browser (NA-1)', async ({
  backend,
  page,
  dav,
}) => {
  await stubWindowOpen(page)
  await put(dav, 'Docs/My Report.pdf', makePdf(REPORT))
  await put(dav, 'Docs/Letter.docx', 'PK')
  const g = await synced(backend, dav)
  const docx = `${dav.server}/f/${await fileId(dav, 'Docs/Letter.docx')}`
  g.write(
    'pages/reading.md',
    [
      `- [by path](${account(dav)}/Docs/My%20Report.pdf)`,
      `- [a letter](${docx})`,
      '- [elsewhere](https://cloud.example.org/f/12345)',
      '- [a site](https://example.org/page)',
    ].join('\n') + '\n',
  )
  await backend.open('/page/reading')
  await links(page).getByRole('link', { name: 'by path' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await expect(page.getByTestId('pdf-viewer').locator('h2')).toHaveText('My Report.pdf')
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('pdf-viewer')).toBeHidden()

  for (const [name, url] of [
    ['a letter', docx],
    ['elsewhere', 'https://cloud.example.org/f/12345'],
    ['a site', 'https://example.org/page'],
  ]) {
    await links(page).getByRole('link', { name }).click()
    await expect.poll(async () => (await opened(page)).at(-1)).toBe(url)
    await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
    await expect(page.getByTestId('epub-reader')).toHaveCount(0)
  }
})

test('a graph without a server opens its Nextcloud-looking links in the browser (NA-1)', async ({
  backend,
  page,
}) => {
  await stubWindowOpen(page)
  backend.write('pages/reading.md', '- [a file](https://cloud.example.org/f/12345)\n')
  await backend.open('/page/reading')
  await links(page).getByRole('link', { name: 'a file' }).click()
  await expect.poll(async () => (await opened(page)).at(-1)).toBe('https://cloud.example.org/f/12345')
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
})

test('an EPUB of the server: highlight, comment, reference and chip; the file changes on the server and the highlight follows (NA-2, NA-4, NA-7)', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Books/novel.epub', novel())
  const g = await synced(backend, dav)
  const id = await fileId(dav, 'Books/novel.epub')
  const link = `${dav.server}/f/${id}`
  g.write('pages/reading.md', `- The [book](${link})\n`)
  await backend.open('/page/reading')
  await links(page).getByRole('link', { name: 'book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await highlight(page, NOVEL.s1)
  await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  await expect
    .poll(() => (g.exists('pages/hls/novel.md') ? g.read('pages/hls/novel.md') : ''))
    .toContain('hl-cfi:')
  // a comment
  await page.getByTestId('pdf-item').first().getByTestId('pdf-add-comment').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('check this')
  await page.keyboard.press('Escape')
  await expect.poll(() => g.read('pages/hls/novel.md')).toMatch(/^ {2}- check this$/m)
  const side = g.read('pages/hls/novel.md')
  // the sidecar names the document by its number, and holds its hash
  expect(side).toContain(`file: ${link}`)
  expect(side).toMatch(/^sha: [0-9a-f]{16}$/m)
  const uid = /\^([a-z0-9]{6})\n {2}hl-cfi::/.exec(side)![1]
  await page.getByTestId('epub-close').click()

  // a reference from a note: the chip names the book and opens it at the highlight
  g.write('journals/2026/2026-10-03.md', `- Remember ((${uid}))\n`)
  await backend.open('/page/2026-10-03')
  const chip = page.getByTestId('pdf-ref-chip')
  await expect(chip).toContainText('novel')
  await chip.click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await expect(page.getByTestId('epub-mark').filter({ hasText: 'dark and stormy' })).toHaveAttribute(
    'data-flash',
    'true',
  )
  await page.getByTestId('epub-close').click()

  // the book changes on the server: a paragraph above the highlighted sentence
  const [first, ...rest] = NOVEL.chapters
  await put(
    dav,
    'Books/novel.epub',
    makeEpub({
      title: NOVEL.title,
      files: NOVEL.files,
      chapters: [
        { ...first, html: '<p>An added opening paragraph about the weather.</p>' + first.html },
        ...rest,
      ],
    }),
  )
  await page.getByRole('tree', { name: 'Blocks' }).locator('.block-ref').first().waitFor()
  await chip.click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, 'An added opening paragraph about the weather.')
  await expect(page.getByTestId('epub-mark').filter({ hasText: 'dark and stormy' })).toHaveCount(1)
  await expect(page.getByTestId('pdf-item-missing')).toHaveCount(0)
  // the same sidecar
  expect(fs.readdirSync(path.join(g.dir, 'pages/hls'))).toEqual(['novel.md'])
})

test('with the server gone the copy of an earlier open still opens, with a note (NA-2)', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Docs/Report.pdf', makePdf(REPORT))
  const g = await synced(backend, dav)
  const id = await fileId(dav, 'Docs/Report.pdf')
  g.write('pages/reading.md', `- [the report](${dav.server}/f/${id})\n`)
  await backend.open('/page/reading')
  await links(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')
  await page.keyboard.press('Escape')

  dav.stop()
  await links(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await expect(page.getByTestId('doc-offline')).toContainText('cannot be reached')
  await expect(page.getByTestId('doc-offline')).toContainText('2026-')
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')
})

test('a document that was never opened says why it cannot be had when the server is gone (NA-2)', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Docs/Report.pdf', makePdf(REPORT))
  const g = await synced(backend, dav)
  const id = await fileId(dav, 'Docs/Report.pdf')
  g.write('pages/reading.md', `- [the report](${dav.server}/f/${id})\n`)
  await backend.open('/page/reading')
  dav.stop()
  await links(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByText(/has not been opened on this device before/)).toBeVisible()
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  // the toast offers the browser
  await expect(page.getByRole('button', { name: 'Open in the browser' })).toBeVisible()
})

test('Keep a copy puts the file in the graph and the old link finds the same sidecar (NA-6)', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Books/novel.epub', novel())
  const g = await synced(backend, dav)
  const id = await fileId(dav, 'Books/novel.epub')
  const link = `${dav.server}/f/${id}`
  g.write('pages/reading.md', `- The [book](${link})\n`)
  await backend.open('/page/reading')
  await links(page).getByRole('link', { name: 'book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await highlight(page, NOVEL.s1)
  await expect
    .poll(() => (g.exists('pages/hls/novel.md') ? g.read('pages/hls/novel.md') : ''))
    .toContain('hl-cfi:')

  await page.getByTestId('doc-keep').click()
  await expect(page.getByTestId('doc-kept')).toContainText('assets/')
  const rel = /assets\/\d{4}\/\d{2}\/[^\s]+\.epub/.exec(await page.getByTestId('doc-kept').innerText())![0]
  expect(g.exists(rel)).toBe(true)
  await expect.poll(() => g.read('pages/hls/novel.md')).toContain(`file: ${rel}`)
  await page.getByTestId('epub-close').click()

  // the old link: the highlight is there, and there is still one sidecar that points at the copy
  await links(page).getByRole('link', { name: 'book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  expect(fs.readdirSync(path.join(g.dir, 'pages/hls'))).toEqual(['novel.md'])
  expect(g.read('pages/hls/novel.md')).toContain(`file: ${rel}`)
})

test('/file on the graph server offers Annotate for a PDF: the link is written and the viewer opens (NA-7)', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Docs/Report.pdf', makePdf(REPORT))
  await put(dav, 'Docs/Letter.docx', 'PK')
  const g = await synced(backend, dav)
  g.write('pages/t.md', '- see\n')
  await backend.open('/page/t')
  await links(page).getByText('see').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /file')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Link a file/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  const picker = page.getByTestId('file-picker')
  await picker.getByTestId('file-source').first().click()
  await picker.getByRole('button', { name: 'Docs', exact: true }).click()
  // only a PDF or an EPUB can be annotated
  await expect(picker.getByTestId('file-annotate')).toHaveCount(1)
  await picker.getByTestId('file-annotate').click()
  await expect(picker).toBeHidden()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
    timeout: 15_000,
  })
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await expect.poll(() => g.read('pages/t.md')).toMatch(/^- see \[Report\.pdf\]\(.*\/f\/\d+\)\n$/)
})

test('a file over 100 MB is refused with a message and the browser is offered (NA-5)', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Docs/Huge.pdf', Buffer.alloc(100 * 1024 * 1024 + 1, 'x'))
  const g = await synced(backend, dav)
  const id = await fileId(dav, 'Docs/Huge.pdf')
  g.write('pages/reading.md', `- [huge](${dav.server}/f/${id})\n`)
  await backend.open('/page/reading')
  await links(page).getByRole('link', { name: 'huge' }).click()
  await expect(page.getByText(/too large to open here/)).toBeVisible()
  await expect(page.getByRole('button', { name: 'Open in the browser' })).toBeVisible()
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
})
