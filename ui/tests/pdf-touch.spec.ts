import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from './fixtures'
import { makePdf, REPORT } from './pdf-fixture'

// PA-8: the PDF viewer on a tablet. The browser is emulated as a touch screen with a coarse pointer and no hover;
// a real iPad (selection by long press and handles, pinch, the Pencil) is an open check in docs/phases/pdf-annotation.md.
test.use({ viewport: { width: 1024, height: 768 }, hasTouch: true, isMobile: true })

async function open(
  backend: Parameters<Parameters<typeof test>[2]>[0]['backend'],
  page: import('@playwright/test').Page,
) {
  fs.mkdirSync(path.join(backend.dir, 'assets'), { recursive: true })
  fs.writeFileSync(path.join(backend.dir, 'assets/report.pdf'), makePdf(REPORT))
  backend.write('pages/reading-notes.md', '- Read the [Report](../assets/report.pdf) today\n')
  await backend.open('/page/reading-notes')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'Report' }).click()
  await expect(page.getByTestId('pdf-page').first().locator('.textLayer')).toContainText('powerhouse', {
    timeout: 10_000,
  })
}

test('the colour bar and the buttons are 44 px for a finger (PA-8)', async ({ backend, page }) => {
  await open(backend, page)
  await page.evaluate(() => {
    const span = document.querySelector('[data-n="1"] .textLayer span[data-i]')!
    const range = document.createRange()
    range.setStart(span.firstChild!, 0)
    range.setEnd(span.firstChild!, 10)
    const sel = window.getSelection()!
    sel.removeAllRanges()
    sel.addRange(range)
  })
  await expect(page.getByTestId('pdf-selbar')).toBeVisible()
  for (const id of ['pdf-color-yellow', 'pdf-color-purple', 'pdf-copy']) {
    const box = (await page.getByTestId(id).boundingBox())!
    expect(box.height, id).toBeGreaterThanOrEqual(44)
    expect(box.width, id).toBeGreaterThanOrEqual(44)
  }
  const next = (await page.getByTestId('pdf-next').boundingBox())!
  expect(next.height).toBeGreaterThanOrEqual(44)
})

test('two fingers moving apart zoom the page (PA-8)', async ({ backend, page, browserName }) => {
  test.skip(browserName === 'webkit', 'a Touch cannot be constructed in WebKit; a real iPad is an open check')
  await open(backend, page)
  await expect(page.getByTestId('pdf-zoom-fit')).toHaveText('100 %')
  await page.getByTestId('pdf-scroller').evaluate((el) => {
    const t = (id: number, x: number, y: number) =>
      new Touch({ identifier: id, target: el, clientX: x, clientY: y })
    const fire = (type: string, touches: Touch[]) =>
      el.dispatchEvent(new TouchEvent(type, { touches, bubbles: true, cancelable: true }))
    fire('touchstart', [t(1, 300, 300), t(2, 400, 300)])
    fire('touchmove', [t(1, 250, 300), t(2, 450, 300)]) // twice as far apart
    fire('touchend', [])
  })
  await expect(page.getByTestId('pdf-zoom-fit')).toHaveText('200 %')
})

test.describe('a narrow window', () => {
  test.use({ viewport: { width: 700, height: 900 } })
  test('the panel is a drawer over the pages (PA-8)', async ({ backend, page }) => {
    await open(backend, page)
    await page.getByTestId('pdf-panel-toggle').click()
    const panel = page.getByTestId('pdf-panel')
    await expect(panel).toBeVisible()
    await expect(panel).toHaveCSS('position', 'absolute')
    const scroller = (await page.getByTestId('pdf-scroller').boundingBox())!
    const drawer = (await panel.boundingBox())!
    // it lies over the pages: it does not take room from them
    expect(drawer.x).toBeLessThan(scroller.x + scroller.width)
    expect(scroller.width).toBeGreaterThan(500)
  })
})
