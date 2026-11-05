import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Frame, type Page } from './fixtures'
import { longChapter, NOVEL, novel } from './epub-fixture'

// EA-9: the EPUB reader on a tablet. The browser is emulated as a touch screen with a coarse pointer; a real iPad
// (selection by long press and handles inside the book, the system's own bar) is an open check in
// docs/phases/epub-annotation.md.
test.use({ viewport: { width: 1024, height: 768 }, hasTouch: true, isMobile: true })

type Backend = Parameters<Parameters<typeof test>[2]>[0]['backend']

async function open(backend: Backend, page: Page, bytes = novel()) {
  fs.mkdirSync(path.join(backend.dir, 'assets/books'), { recursive: true })
  fs.writeFileSync(path.join(backend.dir, 'assets/books/novel.epub'), bytes)
  backend.write('pages/reading-notes.md', '- Read [The book](../assets/books/novel.epub) today\n')
  await backend.open('/page/reading-notes')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'The book' }).click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
}

const frames = (page: Page): Frame[] => page.frames().filter((f) => f !== page.mainFrame())

test('the colour bar and the buttons are 44 px for a finger (EA-9)', async ({ backend, page }) => {
  await open(backend, page)
  await expect
    .poll(async () =>
      (
        await Promise.all(
          frames(page).map((f) =>
            f
              .locator('body')
              .innerText()
              .catch(() => ''),
          ),
        )
      ).join(),
    )
    .toContain(NOVEL.s1)
  await frames(page)[0].evaluate((text) => {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const i = (n as Text).data.indexOf(text)
      if (i < 0) continue
      const r = document.createRange()
      r.setStart(n, i)
      r.setEnd(n, i + text.length)
      const sel = window.getSelection()!
      sel.removeAllRanges()
      sel.addRange(r)
      return
    }
  }, 'dark and stormy')
  await expect(page.getByTestId('pdf-selbar')).toBeVisible()
  for (const id of ['pdf-color-yellow', 'pdf-color-purple', 'pdf-copy']) {
    const box = (await page.getByTestId(id).boundingBox())!
    expect(box.height, id).toBeGreaterThanOrEqual(44)
    expect(box.width, id).toBeGreaterThanOrEqual(44)
  }
  for (const id of ['epub-next', 'epub-prev', 'epub-toc-toggle', 'epub-settings-toggle']) {
    expect((await page.getByTestId(id).boundingBox())!.height, id).toBeGreaterThanOrEqual(44)
  }
})

test('a swipe turns the page (EA-9)', async ({ backend, page, browserName }) => {
  test.skip(browserName === 'webkit', 'a Touch cannot be constructed in WebKit; a real iPad is an open check')
  await open(backend, page, longChapter(60))
  await expect
    .poll(async () =>
      (
        await Promise.all(
          frames(page).map((f) =>
            f
              .locator('body')
              .innerText()
              .catch(() => ''),
          ),
        )
      ).join(),
    )
    .toContain('Paragraph 1:')
  const percent = async () => Number((await page.getByTestId('epub-percent').innerText()).replace(/\D/g, ''))
  const before = await percent()
  await frames(page)[0].evaluate(() => {
    const t = (x: number) => new Touch({ identifier: 1, target: document.body, clientX: x, clientY: 300 })
    document.dispatchEvent(new TouchEvent('touchstart', { touches: [t(400)], bubbles: true }))
    document.dispatchEvent(new TouchEvent('touchend', { changedTouches: [t(200)], bubbles: true }))
  })
  await expect.poll(percent).toBeGreaterThan(before)
})

test.describe('a narrow window', () => {
  test.use({ viewport: { width: 700, height: 900 } })
  test('the panel and the contents are drawers over the page (EA-9)', async ({ backend, page }) => {
    await open(backend, page)
    await page.getByTestId('epub-panel-toggle').click()
    const panel = page.getByTestId('pdf-panel')
    await expect(panel).toBeVisible()
    await expect(panel).toHaveCSS('position', 'absolute')
    await page.getByTestId('epub-panel-toggle').click()
    await page.getByTestId('epub-toc-toggle').click()
    await expect(page.getByTestId('epub-toc')).toHaveCSS('position', 'absolute')
    const reader = (await page.getByTestId('epub-page').boundingBox())!
    expect(reader.width).toBeGreaterThan(500)
  })
})
