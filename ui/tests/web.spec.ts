import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from './fixtures'
import { article, FIRST, pageServer, SECOND, type PageServer } from './web-fixture'
import { chapterFrames, highlight, reads } from './epub-reader-helpers'

// WA-1..WA-10, the whole path: clip a page from the app, read and highlight it with the EPUB reader, come back through a
// reference, clip again after the page changed. The dev server runs with --allow-local-fetch (`clip-refused.spec.ts`
// runs without it).
test.use({ localFetch: true })

let web: PageServer
test.beforeEach(async () => {
  web = await pageServer()
})
test.afterEach(async () => {
  await web.close()
})

type Backend = Parameters<Parameters<typeof test>[2]>[0]['backend']

/** Clips `address` with `/clip` at the end of the block `start` of the page `notes`, then leaves the editor. */
async function clipFromNotes(page: Page, address: string, firstTime = true) {
  const tree = page.getByRole('tree', { name: 'Blocks' })
  // at the left edge of the word: once the block holds a link, a click in the middle would follow it
  await tree
    .getByText('start')
    .first()
    .click({ position: { x: 2, y: 5 } })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /clip')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Clip a web page…', { timeout: 5000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await page.getByTestId('clip-address').fill(address)
  await page.getByTestId('clip-go').click()
  if (!firstTime) {
    await expect(page.getByTestId('clip-again')).toBeVisible()
    await page.getByTestId('clip-go').click()
  }
  // a clip with notes (a picture left out) waits for the link to be accepted
  const insert = page.getByTestId('clip-insert')
  await expect(async () => {
    if (await insert.isVisible()) await insert.click()
    await expect(page.getByTestId('clip-dialog')).toHaveCount(0, { timeout: 500 })
  }).toPass({ timeout: 15_000 })
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('block-editor')).toHaveCount(0)
}

/** The sidecar of the only clip: its page file as the app wrote it. */
function sidecarOf(backend: Backend): { name: string; text: string } {
  const dir = path.join(backend.dir, 'pages/hls')
  const [file] = fs.existsSync(dir) ? fs.readdirSync(dir) : []
  return { name: file, text: file ? backend.read(`pages/hls/${file}`) : '' }
}

async function openClip(page: Page, title = /The Lantern/) {
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: title }).first().click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
}

test('clip, read, highlight, comment, reference: the chip opens the clip at the highlight (WA-5, WA-6)', async ({
  backend,
  page,
}) => {
  backend.write('pages/notes.md', '- start\n')
  await backend.open('/page/notes')
  await clipFromNotes(page, `${web.base}/article`)
  await openClip(page)
  await reads(page, FIRST)
  await highlight(page, FIRST)
  await expect(page.getByTestId('pdf-item')).toHaveCount(1)
  await expect.poll(() => sidecarOf(backend).text).toContain('hl-cfi:: epubcfi(')
  // a comment
  await page.getByTestId('pdf-item').first().getByTestId('pdf-add-comment').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('check the date of this')
  await page.keyboard.press('Escape')
  await expect.poll(() => sidecarOf(backend).text).toMatch(/^ {2}- check the date of this$/m)
  const { name, text } = sidecarOf(backend)
  expect(text).toMatch(/^file: assets\/web\/the-lantern[a-z-]*-[0-9a-f]{8}\.epub$/m)
  const uid = /\^([a-z0-9]{6})\n {2}hl-cfi::/.exec(text)![1]
  await page.getByTestId('epub-close').click()
  // a reference from a note
  backend.write('journals/2026/2026-10-03.md', `- As ((${uid})) says\n`)
  await backend.open('/page/2026-10-03')
  await expect(page.getByRole('tree', { name: 'Blocks' }).locator('.block-ref')).toContainText('harbour town')
  const chip = page.getByTestId('pdf-ref-chip')
  await expect(chip).toContainText('.epub')
  await chip.click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, FIRST)
  await expect(page.getByTestId('epub-mark').filter({ hasText: 'harbour town' })).toHaveAttribute(
    'data-flash',
    'true',
  )
  // the sidecar page stays hidden
  expect(name).toMatch(/\.md$/)
})

test('clipped again after the page changed: the highlight is drawn on the same words (WA-6)', async ({
  backend,
  page,
}) => {
  backend.write('pages/notes.md', '- start\n')
  await backend.open('/page/notes')
  await clipFromNotes(page, `${web.base}/article`)
  await openClip(page)
  await reads(page, FIRST)
  await highlight(page, FIRST)
  await expect.poll(() => sidecarOf(backend).text).toContain('hl-cfi:: epubcfi(')
  const before = sidecarOf(backend)
  await page.getByTestId('epub-close').click()

  // a paragraph is added above the highlighted sentence
  web.articleHtml = article('The Lantern', '<p>An added paragraph about the ferry timetable.</p>')
  await clipFromNotes(page, `${web.base}/article`, false)
  await openClip(page)
  await reads(page, 'An added paragraph about the ferry timetable.')
  // the reader may open where the reading stopped; the entry of the panel leads to the highlight
  await page.getByTestId('pdf-item').first().getByTestId('pdf-item-quote').click()
  await expect(page.getByTestId('epub-mark')).toHaveCount(1)
  await expect(page.getByTestId('epub-mark')).toHaveText(FIRST)
  await expect(page.getByTestId('pdf-item-missing')).toHaveCount(0)
  // the same sidecar, the same highlight block
  expect(sidecarOf(backend).name).toBe(before.name)
  expect(sidecarOf(backend).text).toContain(`^${/\^([a-z0-9]{6})\n {2}hl-cfi::/.exec(before.text)![1]}`)
})

test('clipped again with the highlighted sentence changed: it is listed as not found (WA-6)', async ({
  backend,
  page,
}) => {
  backend.write('pages/notes.md', '- start\n')
  await backend.open('/page/notes')
  await clipFromNotes(page, `${web.base}/article`)
  await openClip(page)
  await reads(page, FIRST)
  await highlight(page, FIRST)
  await expect.poll(() => sidecarOf(backend).text).toContain('hl-cfi:: epubcfi(')
  await page.getByTestId('epub-close').click()

  web.articleHtml = article().replace(
    'woke slowly, as it always did in the grey weeks',
    'was already awake, as it never was in the green weeks',
  )
  await clipFromNotes(page, `${web.base}/article`, false)
  await openClip(page)
  await reads(page, SECOND)
  await expect(page.getByTestId('pdf-item-missing')).toHaveCount(1)
  await expect(page.getByTestId('epub-mark')).toHaveCount(0)
  // the quote is still in the sidecar: nothing was thrown away
  expect(sidecarOf(backend).text).toContain('woke slowly')
})

test('a hostile page: nothing runs when the clip is read, no request leaves for its remote things (WA-3, WA-4)', async ({
  backend,
  page,
}) => {
  const leaks: string[] = []
  await page.route(/leak\.invalid/, (route) => {
    leaks.push(route.request().url())
    return route.abort()
  })
  backend.write('pages/notes.md', '- start\n')
  await backend.open('/page/notes')
  await clipFromNotes(page, `${web.base}/hostile`)
  await openClip(page, /Hostile/)
  await reads(page, 'Safe text before the traps.')
  const frame = chapterFrames(page).find((f) => f.url().startsWith('blob:'))!
  // the clean book holds the article and nothing active
  expect(
    await frame.evaluate(() =>
      Array.from(document.querySelectorAll('script, iframe, form, link[rel=stylesheet], object, embed'))
        // the reader's own style sheet is a blob: of the app
        .filter((e) => !(e.getAttribute('href') ?? '').startsWith('blob:'))
        .map((e) => e.outerHTML.slice(0, 200))
        .join(),
    ),
  ).toBe('')
  expect(await frame.evaluate(() => document.querySelectorAll('[onclick], [onerror]').length)).toBe(0)
  await frame.getByText('Safe text before the traps.').click()
  await page.waitForTimeout(500)
  expect(await page.evaluate(() => (window as unknown as { __pwned?: string }).__pwned)).toBeUndefined()
  expect(leaks).toEqual([])
  // no javascript: link survived
  expect(
    await frame.evaluate(() => document.querySelector('a[href^="javascript"]')?.outerHTML ?? null),
  ).toBeNull()
})
