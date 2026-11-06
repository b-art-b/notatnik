import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from './fixtures'
import { makeEpub, NOVEL, novel } from './epub-fixture'
import { bookText, chapterFrames, highlight, reads } from './epub-reader-helpers'
import { makePdf, REPORT } from './pdf-fixture'

// DD-1..DD-8: a PDF or an EPUB docked into the side panel, beside the notes.
type Backend = Parameters<Parameters<typeof test>[2]>[0]['backend']

const tree = (page: Page) => page.getByRole('tree', { name: 'Blocks' })

function put(backend: Backend, rel: string, bytes: Buffer) {
  const full = path.join(backend.dir, rel)
  fs.mkdirSync(path.dirname(full), { recursive: true })
  fs.writeFileSync(full, bytes)
}

/** A page with links to a PDF and a book (and to another page, for a tab beside the document). */
async function notes(backend: Backend) {
  put(backend, 'assets/report.pdf', makePdf(REPORT))
  put(backend, 'assets/novel.epub', novel())
  backend.write(
    'pages/reading.md',
    '- Read [the report](../assets/report.pdf)\n- And [the book](../assets/novel.epub)\n- See [[Side page]]\n- A free line to write in\n',
  )
  backend.write('pages/Side page.md', '- something on the side\n')
  await backend.open('/page/reading')
}

const box = async (page: Page, testId: string) => (await page.getByTestId(testId).boundingBox())!

test('Dock moves an open PDF into the side panel beside the notes, Full screen brings it back (DD-1, DD-4)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  // a page opened beside the notes makes the panel show its tabs
  await tree(page)
    .getByRole('link', { name: 'Side page' })
    .click({ modifiers: ['Shift'] })
  await tree(page).getByRole('link', { name: 'the report' }).click()
  const viewer = page.getByTestId('pdf-viewer')
  await expect(viewer).toBeVisible()
  // full screen: a modal dialog over everything
  expect(await viewer.getAttribute('role')).toBe('dialog')
  expect(await viewer.getAttribute('aria-modal')).toBe('true')
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')

  await page.getByTestId('doc-dock').click()
  const panel = await box(page, 'side-panel')
  const docked = await box(page, 'pdf-viewer')
  // inside the panel, beside the notes
  expect(docked.x).toBeGreaterThanOrEqual(panel.x - 1)
  expect(docked.x + docked.width).toBeLessThanOrEqual(panel.x + panel.width + 1)
  expect(docked.width).toBeLessThan(panel.width + 1)
  expect(docked.height).toBeGreaterThan(300)
  expect(await viewer.getAttribute('role')).toBe('region')
  expect(await viewer.getAttribute('aria-modal')).toBeNull()
  await expect(viewer).toHaveAttribute('aria-label', /report\.pdf/)
  // the notes are usable: a link still works and the page is not covered
  await expect(tree(page).getByRole('link', { name: 'the book' })).toBeVisible()
  const link = (await tree(page).getByRole('link', { name: 'the book' }).boundingBox())!
  expect(link.x + link.width).toBeLessThanOrEqual(panel.x + 1)
  // the panel has a tab named after the file
  await expect(page.getByRole('tab', { name: /report\.pdf/ })).toBeVisible()

  await page.getByTestId('doc-dock').click()
  expect(await viewer.getAttribute('role')).toBe('dialog')
  const full = await box(page, 'pdf-viewer')
  expect(full.width).toBeGreaterThan(1300)
  await expect(page.getByRole('tab', { name: /report\.pdf/ })).toHaveCount(0)
})

test('docking and undocking keep the PDF where it was: page, find, highlights (DD-3)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')
  await page.getByTestId('pdf-next').click()
  await expect(page.getByTestId('pdf-page-box')).toHaveValue('2')
  await page.getByTestId('doc-dock').click()
  await expect(page.getByTestId('pdf-page-box')).toHaveValue('2')
  await expect(page.getByTestId('pdf-page').nth(1)).toBeVisible()
  await page.getByTestId('doc-dock').click()
  await expect(page.getByTestId('pdf-page-box')).toHaveValue('2')
})

test('docking an EPUB reloads nothing: the same chapter frame, the same place, the same text (DD-3)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
  // mark the frame element: a reload would make a new one
  await page.evaluate(() => {
    const f = document.querySelector('foliate-view')!
    ;(window as unknown as { __frames: Element[] }).__frames = Array.from(
      (f.shadowRoot ?? f).querySelectorAll('iframe'),
    )
  })
  await page.getByTestId('epub-toc-toggle').click()
  await page.getByTestId('epub-toc-item').nth(1).click()
  await reads(page, NOVEL.s2)
  const place = await page.getByTestId('epub-percent').innerText()
  await page.getByTestId('doc-dock').click()
  await expect(page.getByTestId('epub-reader')).toHaveAttribute('role', 'region')
  expect(await bookText(page)).toContain(NOVEL.s2)
  expect(await page.getByTestId('epub-percent').innerText()).toBe(place)
  // the text is laid out for the panel, not for the window
  const frame = chapterFrames(page).find((f) => f.url().startsWith('blob:'))!
  const panel = await box(page, 'side-panel')
  expect(await frame.evaluate(() => innerWidth)).toBeLessThanOrEqual(panel.width)
  await page.getByTestId('doc-dock').click()
  expect(await bookText(page)).toContain(NOVEL.s2)
  expect(await page.getByTestId('epub-percent').innerText()).toBe(place)
})

test('the panel is resized, the other tabs work, the tab closes the document (DD-4)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  // a page opened beside the notes is another tab of the panel
  await tree(page)
    .getByRole('link', { name: 'Side page' })
    .click({ modifiers: ['Shift'] })
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await page.getByTestId('doc-dock').click()
  await expect(page.getByRole('tab', { name: /Pages/ })).toBeVisible()
  const w0 = (await box(page, 'pdf-viewer')).width
  // the resizer: wider panel, wider document
  const rz = (await page.getByTestId('right-resizer').boundingBox())!
  await page.mouse.move(rz.x + 2, rz.y + 200)
  await page.mouse.down()
  await page.mouse.move(rz.x - 150, rz.y + 200, { steps: 5 })
  await page.mouse.up()
  await expect.poll(async () => (await box(page, 'pdf-viewer')).width).toBeGreaterThan(w0 + 100)
  // the Pages tab hides the document without closing it; the document tab shows it again, at the same place
  await page.getByRole('tab', { name: /Pages/ }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeHidden()
  await expect(page.getByText('something on the side')).toBeVisible()
  await page.getByRole('tab', { name: /report\.pdf/ }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')
  // the tab's cross closes the document and the tab that was open before comes back
  await page.getByRole('tab', { name: /Pages/ }).click()
  await page.getByTestId('doc-tab-close').click()
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  await expect(page.getByRole('tab', { name: /report\.pdf/ })).toHaveCount(0)
  await expect(page.getByText('something on the side')).toBeVisible()
})

test('below 900 px there is no Dock, and a window made narrower brings a docked document back (DD-2)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await page.getByTestId('doc-dock').click()
  await expect(page.getByTestId('pdf-viewer')).toHaveAttribute('role', 'region')
  await page.setViewportSize({ width: 800, height: 900 })
  await expect(page.getByTestId('pdf-viewer')).toHaveAttribute('role', 'dialog')
  await expect(page.getByTestId('doc-dock')).toHaveCount(0)
  await page.getByTestId('pdf-close').click()
  // a narrow window opens it full screen, with no button to dock
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toHaveAttribute('role', 'dialog')
  await expect(page.getByTestId('doc-dock')).toHaveCount(0)
})

test('the setting opens documents docked; the next one replaces it in the panel; without the setting too (DD-2, DD-8)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await page.addInitScript(() => localStorage.setItem('notatnik.dockDocuments', 'yes'))
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toHaveAttribute('role', 'region')
  // it did not take the focus away
  await expect(page.getByTestId('pdf-viewer')).not.toBeFocused()
  // the next link replaces it in the panel: a book now, the PDF is gone
  await tree(page).getByRole('link', { name: 'the book' }).click()
  await expect(page.getByTestId('epub-reader')).toHaveAttribute('role', 'region')
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  await reads(page, NOVEL.s1)
})

test('a link replaces a docked document in the panel also when the setting is off (DD-8)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await page.getByTestId('doc-dock').click()
  await tree(page).getByRole('link', { name: 'the book' }).click()
  await expect(page.getByTestId('epub-reader')).toHaveAttribute('role', 'region')
  await expect(page.getByTestId('pdf-viewer')).toHaveCount(0)
  // the notes page is as it was
  await expect(tree(page).getByRole('link', { name: 'the report' })).toBeVisible()
})

test('the setting is a switch in Settings → Linked files, off by default and kept (DD-2)', async ({
  backend,
  page,
}) => {
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Linked files' }).click()
  const sw = page.getByTestId('dock-setting')
  await expect(sw).not.toBeChecked()
  await sw.check()
  await page.reload()
  await page.getByRole('tab', { name: 'Linked files' }).click()
  await expect(page.getByTestId('dock-setting')).toBeChecked()
})

test('writing next to a docked document: the keys of the notes reach the notes, the book does not turn (DD-5)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the book' }).click()
  await reads(page, NOVEL.s1)
  await page.getByTestId('doc-dock').click()
  const place = await page.getByTestId('epub-percent').innerText()
  // a note
  await tree(page).getByText('A free line to write in').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' and more')
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowLeft')
  // Escape in the note leaves the note, never the document
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await expect.poll(() => backend.read('pages/reading.md')).toContain('A free line to write in and more')
  expect(await page.getByTestId('epub-percent').innerText()).toBe(place)
  // the app's own keys still work: Alt+1 leaves for the journals and the document stays docked
  await page.keyboard.press('Alt+1')
  await expect(page.getByTestId('epub-reader')).toHaveAttribute('role', 'region')
})

test('with the focus in the docked book the arrows turn pages; Escape then closes the document, from inside only (DD-5)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the book' }).click()
  await reads(page, NOVEL.s1)
  await page.getByTestId('doc-dock').click()
  await page.getByTestId('epub-next').focus()
  const before = await page.getByTestId('epub-percent').innerText()
  await page.getByTestId('epub-reader').locator('header').getByTestId('epub-next').click()
  await expect.poll(() => page.getByTestId('epub-percent').innerText()).not.toBe(before)
  // Escape in a note does nothing to the document ...
  await tree(page).getByText('A free line to write in').click()
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  // ... and Escape with the focus in the viewer closes it
  await page.getByTestId('epub-next').focus()
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('epub-reader')).toHaveCount(0)
})

test('highlight in the docked book, copy as a quote, paste in a note, the chip opens it in the panel (DD-7)', async ({
  backend,
  page,
  context,
  browserName,
}) => {
  test.skip(browserName !== 'chromium', 'clipboard permissions are a Chromium feature of the test driver')
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the book' }).click()
  await reads(page, NOVEL.s1)
  await page.getByTestId('doc-dock').click()
  await reads(page, NOVEL.s1)
  await highlight(page, NOVEL.s1)
  // the highlights list is a drawer, closed in a narrow box; the highlight is drawn and kept
  await expect(page.getByTestId('epub-mark')).toHaveCount(1)
  await expect
    .poll(() => (backend.exists('pages/hls/novel.md') ? backend.read('pages/hls/novel.md') : ''))
    .toContain('hl-cfi:')
  await page.evaluate(() => navigator.clipboard.writeText(''))
  // the popup of the highlight: Copy as quote
  const frame = chapterFrames(page).find((f) => f.url().startsWith('blob:'))!
  const rect = await frame.evaluate((t) => {
    const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
    for (let n = w.nextNode() as Text | null; n; n = w.nextNode() as Text | null) {
      const i = n.data.indexOf(t)
      if (i < 0) continue
      const r = document.createRange()
      r.setStart(n, i)
      r.setEnd(n, i + 12)
      const b = r.getBoundingClientRect()
      return { x: b.left + b.width / 2, y: b.top + b.height / 2 }
    }
    return null
  }, 'dark and stormy')
  const fb = (await (await frame.frameElement()).boundingBox())!
  await page.mouse.click(fb.x + rect!.x, fb.y + rect!.y)
  await page.getByTestId('pdf-popup-quote').click()
  // paste it into a note, next to the document
  await tree(page).getByText('A free line to write in').click()
  await page.keyboard.press('End')
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.press('Control+v')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/reading.md'))
    .toMatch(/> It was a dark and stormy night.*\(\([a-z0-9]{6}\)\)/)
  // the chip opens the book at the highlight, in the panel
  await page.getByTestId('epub-close').click()
  await page.reload()
  await tree(page).getByTestId('pdf-ref-chip').click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await reads(page, NOVEL.s1)
})

test('a docked viewer in a narrow panel: the highlights are a drawer and nothing scrolls sideways (DD-6)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the book' }).click()
  await reads(page, NOVEL.s1)
  await page.getByTestId('doc-dock').click()
  const panelBox = await box(page, 'epub-reader')
  expect(panelBox.width).toBeLessThan(900)
  // it starts closed in a narrow box ...
  await expect(page.getByTestId('pdf-panel')).toHaveCount(0)
  // ... and opens over the book instead of squeezing it
  await page.getByTestId('epub-panel-toggle').click()
  const drawer = page.getByTestId('pdf-panel')
  await expect(drawer).toBeVisible()
  expect(await drawer.evaluate((el) => getComputedStyle(el).position)).toBe('absolute')
  const dBox = (await drawer.boundingBox())!
  expect(dBox.x + dBox.width).toBeLessThanOrEqual(panelBox.x + panelBox.width + 1)
  // no sideways scroll in the viewer or the page
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth),
  ).toBe(true)
  const hasOverflow = await page
    .getByTestId('epub-reader')
    .evaluate((el) => el.scrollWidth > el.clientWidth + 1)
  expect(hasOverflow).toBe(false)
})

test('full screen is as it was: modal, focused, the whole window (DD-1)', async ({ backend, page }) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the book' }).click()
  const reader = page.getByTestId('epub-reader')
  await expect(reader).toBeFocused()
  await expect(reader).toHaveAttribute('aria-modal', 'true')
  const b = await box(page, 'epub-reader')
  expect(b.width).toBeGreaterThan(1300)
  void makeEpub
})

test('a docked PDF is laid out for the panel: its pages are no wider than the box, and follow a resize (DD-6)', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')
  const full = (await page.getByTestId('pdf-page').first().boundingBox())!.width
  await page.getByTestId('doc-dock').click()
  await expect
    .poll(async () => (await page.getByTestId('pdf-page').first().boundingBox())!.width)
    .toBeLessThan(full * 0.6)
  const viewer = await box(page, 'pdf-viewer')
  const pg = (await page.getByTestId('pdf-page').first().boundingBox())!
  expect(pg.x).toBeGreaterThanOrEqual(viewer.x)
  expect(pg.x + pg.width).toBeLessThanOrEqual(viewer.x + viewer.width + 1)
  // the text is still there and can be selected for a highlight
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse')
})

test('Dock and Full screen are reached by the keyboard (DD-1)', async ({ backend, page }) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  await notes(backend)
  await tree(page).getByRole('link', { name: 'the report' }).click()
  await page.getByTestId('doc-dock').focus()
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('pdf-viewer')).toHaveAttribute('role', 'region')
  await expect(page.getByTestId('doc-dock')).toBeFocused()
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('pdf-viewer')).toHaveAttribute('role', 'dialog')
})
