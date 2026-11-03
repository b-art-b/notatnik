import type { Page } from '@playwright/test'
import { expect, test } from './fixtures'
import {
  SVG_LINK,
  drawingOf,
  pointerStroke,
  slashDraw,
  stroke,
  withoutSheet,
  wobble,
  type Backend,
} from './drawing-helpers'

// DR-13 to DR-24: what the drawing editor has beside the pens: presets, Pen only, snapping, zoom, background, focus,
// drafts, other devices' copies, and getting a drawing out. (Make space is in drawing.spec.ts.)

const editor = (page: Page) => page.getByTestId('drawing-editor')
const more = async (page: Page, item: string) => {
  await page.getByTestId('drawing-more').click()
  await page.getByTestId(item).click()
}
const strokeCount = (svg: string) => (withoutSheet(svg).match(/<path/g) ?? []).length

/** Done on a new drawing, and the saved SVG of the page's block. */
async function finish(page: Page, backend: Backend, pageName: string) {
  await page.getByTestId('drawing-done').click()
  await expect(editor(page)).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read(`pages/${pageName}.md`)).toMatch(SVG_LINK)
  return drawingOf(backend, `pages/${pageName}.md`)
}

test('DR-15: a preset sets colour, width and opacity of the pen; a long press or the right button stores a pen', async ({
  backend,
  page,
}) => {
  backend.write('pages/x1.md', '- start\n')
  await slashDraw(page, backend, 'x1', 'start')
  // the red pen, then the yellow highlighter
  await page.getByTestId('drawing-preset-2').click()
  await expect(page.getByTestId('drawing-preset-2')).toHaveAttribute('aria-pressed', 'true')
  await stroke(page, wobble)
  await page.getByTestId('drawing-preset-4').click()
  await expect(page.getByTestId('drawing-preset-4')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('drawing-preset-2')).toHaveAttribute('aria-pressed', 'false')
  await stroke(
    page,
    wobble.map(([x, y]) => [x, y + 60] as [number, number]),
  )
  // slot 1 takes the pen that is in use now: the highlighter
  await page.getByTestId('drawing-preset-0').click({ button: 'right' })
  await expect(page.getByText('Pen 1 stored.')).toBeVisible()
  expect(
    await page.evaluate(() => JSON.parse(localStorage.getItem('notatnik.drawing.presets') ?? '[]')[0]),
  ).toEqual({ color: '#ffd400', width: 18, alpha: 0.4 })
  const { svg } = await finish(page, backend, 'x1')
  const paths = withoutSheet(svg).match(/<path[^>]*>/g) ?? []
  expect(paths).toHaveLength(2)
  expect(paths[0]).toMatch(/#d11f1f/i)
  // yellow at 40 % opacity: the last byte of the colour, 0x66
  expect(paths[1]).toMatch(/#ffd40066/i)
})

test('DR-17: Pen only makes a finger move the canvas; switched off it stays off when a stylus draws', async ({
  backend,
  page,
}) => {
  backend.write('pages/x2.md', '- start\n')
  await slashDraw(page, backend, 'x2', 'start')
  const toggle = page.getByTestId('drawing-pen-only')
  await expect(toggle).toHaveAttribute('aria-pressed', 'false')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-pressed', 'true')
  await pointerStroke(page, 'touch', 21, wobble)
  // a finger moved the canvas and drew nothing; the pen draws
  await pointerStroke(page, 'pen', 22, wobble)
  expect(await page.evaluate(() => localStorage.getItem('notatnik.drawing.stylus'))).toBe('1')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-pressed', 'false')
  expect(await page.evaluate(() => localStorage.getItem('notatnik.drawing.stylus'))).toBe('0')
  // a stylus does not switch it on again, and the finger draws
  await pointerStroke(
    page,
    'pen',
    23,
    wobble.map(([x, y]) => [x, y + 50] as [number, number]),
  )
  await expect(toggle).toHaveAttribute('aria-pressed', 'false')
  await pointerStroke(
    page,
    'touch',
    24,
    wobble.map(([x, y]) => [x, y + 100] as [number, number]),
  )
  const { svg } = await finish(page, backend, 'x2')
  expect(strokeCount(svg)).toBe(3)
})

test('DR-18: with "snap shapes" on, a held-still stroke becomes a clean line; it is off to start with', async ({
  backend,
  page,
}) => {
  backend.write('pages/x3.md', '- start\n')
  await slashDraw(page, backend, 'x3', 'start')
  await page.getByTestId('drawing-more').click()
  await expect(page.getByTestId('drawing-snap')).toHaveAttribute('aria-checked', 'false')
  await page.getByTestId('drawing-snap').click()
  await stroke(page, wobble, 1600)
  await page.getByTestId('drawing-more').click()
  await expect(page.getByTestId('drawing-snap')).toHaveAttribute('aria-checked', 'true')
  await page.keyboard.press('Escape')
  const { svg } = await finish(page, backend, 'x3')
  const d =
    /<path d="([^"]+)" fill="none"/.exec(withoutSheet(svg))?.[1] ??
    /<path d="([^"]+)"/.exec(withoutSheet(svg))![1]
  // the 24 points of the wobble became a line of a few: not one piece per point
  expect((d.match(/[qQcCLl]/g) ?? []).length).toBeLessThan(10)
  expect(
    await page.evaluate(() => JSON.parse(localStorage.getItem('notatnik.drawing.prefs') ?? '{}').snapOnHold),
  ).toBe(true)
})

test('DR-19: zoom in and out, back to 100 %, and fit the drawing in the window', async ({
  backend,
  page,
}) => {
  backend.write('pages/x4.md', '- start\n')
  await slashDraw(page, backend, 'x4', 'start')
  const label = page.getByTestId('drawing-zoom')
  await expect(label).toHaveText('100 %')
  await page.getByTestId('drawing-zoom-in').click()
  await expect(label).toHaveText('125 %')
  await page.getByTestId('drawing-zoom-out').click()
  await page.getByTestId('drawing-zoom-out').click()
  await expect(label).toHaveText('80 %')
  await label.click()
  await expect(label).toHaveText('100 %')
  // a small drawing is shown larger than it was, a big one smaller
  await stroke(page, wobble)
  await page.getByTestId('drawing-fit').click()
  await expect(label).not.toHaveText('100 %')
  const pct = Number((await label.textContent())!.replace(/\D/g, ''))
  expect(pct).toBeGreaterThan(100)
})

test('DR-20: the background is a grid of three sizes or blank, per drawing, and is saved with it', async ({
  backend,
  page,
}) => {
  backend.write('pages/x5.md', '- start\n')
  await slashDraw(page, backend, 'x5', 'start')
  await page.getByTestId('drawing-more').click()
  await expect(page.getByTestId('drawing-bg-medium')).toHaveAttribute('aria-checked', 'true')
  await page.getByTestId('drawing-bg-large').click()
  await page.getByTestId('drawing-more').click()
  await expect(page.getByTestId('drawing-bg-large')).toHaveAttribute('aria-checked', 'true')
  await expect(page.getByTestId('drawing-bg-medium')).toHaveAttribute('aria-checked', 'false')
  await page.keyboard.press('Escape')
  await stroke(page, wobble)
  const big = await finish(page, backend, 'x5')
  expect(big.svg).toMatch(/js-draw-image-background-grid-50/)

  backend.write('pages/x5b.md', '- start\n')
  await slashDraw(page, backend, 'x5b', 'start')
  await more(page, 'drawing-bg-blank')
  await stroke(page, wobble)
  const blank = await finish(page, backend, 'x5b')
  expect(blank.svg).toContain('js-draw-image-background')
  expect(blank.svg).not.toMatch(/js-draw-image-background-grid/)
  // an old drawing opens with the background it was saved with
  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  await page.getByTestId('drawing-more').click()
  await expect(page.getByTestId('drawing-bg-blank')).toHaveAttribute('aria-checked', 'true')
})

test('DR-21: focus hides everything but the canvas; the toolbar can be below the canvas', async ({
  backend,
  page,
}) => {
  backend.write('pages/x6.md', '- start\n')
  await slashDraw(page, backend, 'x6', 'start')
  await more(page, 'drawing-focus')
  await expect(editor(page).locator('header')).toBeHidden()
  await expect(page.getByTestId('drawing-hint')).toBeHidden()
  await expect(page.getByTestId('drawing-focus-pill')).toBeVisible()
  await page.getByTestId('drawing-focus-exit').click()
  await expect(editor(page).locator('header')).toBeVisible()

  const canvas = editor(page).locator('canvas').first()
  const toolbar = editor(page).locator('.toolbar-root').first()
  const above = (await toolbar.boundingBox())!.y < (await canvas.boundingBox())!.y
  expect(above).toBe(true)
  await more(page, 'drawing-toolbar-bottom')
  await expect.poll(async () => (await toolbar.boundingBox())!.y > (await canvas.boundingBox())!.y).toBe(true)
  // and the toolbar still works there
  await page.getByTestId('drawing-preset-2').click()
  await stroke(page, wobble)
  const { svg } = await finish(page, backend, 'x6')
  expect(svg).toMatch(/#d11f1f/i)
})

test('DR-16: the selection is a lasso: a loop around one stroke selects it, and Delete takes it away', async ({
  backend,
  page,
}) => {
  backend.write('pages/x7.md', '- start\n')
  await slashDraw(page, backend, 'x7', 'start')
  // left stroke, right stroke
  await stroke(page, [
    [-260, 0],
    [-140, 0],
  ])
  await stroke(page, [
    [100, 0],
    [220, 0],
  ])
  await editor(page)
    .getByRole('button', { name: /^Select/ })
    .first()
    .click()
  const box = (await editor(page).locator('canvas').first().boundingBox())!
  const cx = box.x + box.width / 2
  const cy = box.y + box.height / 2 + 40
  // a loop around the left stroke that ends where it started: as a rectangle this would select nothing
  await page.mouse.move(cx - 300, cy - 40)
  await page.mouse.down()
  for (const [x, y] of [
    [-100, -40],
    [-100, 40],
    [-300, 40],
    [-300, -40],
  ])
    await page.mouse.move(cx + x, cy + y, { steps: 4 })
  await page.mouse.up()
  await page.keyboard.press('Delete')
  const { svg } = await finish(page, backend, 'x7')
  expect(strokeCount(svg)).toBe(1)
  // the stroke that is left is the right one
  const x = Number(/<path d="M(-?[\d.]+)/.exec(withoutSheet(svg))![1])
  expect(x).toBeGreaterThan(0)
})

test('DR-13: a drawing that was not saved is kept, offered again after a restart, and gone once saved', async ({
  backend,
  page,
}) => {
  const drafts = () =>
    page.evaluate(
      () =>
        new Promise<number>((resolve) => {
          const req = indexedDB.open('notatnik-drawing', 1)
          req.onupgradeneeded = () => req.result.createObjectStore('drafts')
          req.onsuccess = () => {
            const tx = req.result.transaction('drafts')
            const c = tx.objectStore('drafts').count()
            c.onsuccess = () => {
              req.result.close()
              resolve(c.result)
            }
          }
        }),
    )
  backend.write('pages/x8.md', '- start\n')
  await slashDraw(page, backend, 'x8', 'start')
  await stroke(page, wobble)
  // written a few seconds after the change, not at every stroke
  await expect.poll(drafts, { timeout: 15_000 }).toBe(1)
  // the app stops (the page is gone with the unsaved drawing)
  await page.reload()
  await slashDraw(page, backend, 'x8', 'start')
  await expect(page.getByTestId('drawing-draft-banner')).toBeVisible()
  await page.getByTestId('drawing-draft-restore').click()
  await expect(page.getByTestId('drawing-draft-banner')).toHaveCount(0)
  const { svg } = await finish(page, backend, 'x8')
  expect(strokeCount(svg)).toBe(1)
  // saved: no draft is left to be offered
  await expect.poll(drafts).toBe(0)

  // a draft can also be thrown away
  backend.write('pages/x8b.md', '- start\n')
  await slashDraw(page, backend, 'x8b', 'start')
  await stroke(page, wobble)
  await expect.poll(drafts, { timeout: 15_000 }).toBe(1)
  await page.reload()
  await slashDraw(page, backend, 'x8b', 'start')
  await page.getByTestId('drawing-draft-discard').click()
  await expect(page.getByTestId('drawing-draft-banner')).toHaveCount(0)
  await expect.poll(drafts).toBe(0)
})

test('DR-23: a copy another device left of a drawing: add its strokes, use it, or keep this one', async ({
  backend,
  page,
}) => {
  const copyName = (n: string) =>
    `assets/drawings/${n.replace('.svg', '')} (conflicted copy 2026-10-03 120000).svg`
  // this device: one stroke; the other device (the copy): the same stroke and another
  backend.write('pages/x9.md', '- start\n')
  await slashDraw(page, backend, 'x9', 'start')
  await stroke(page, wobble)
  const first = await finish(page, backend, 'x9')
  const mine = first.svg
  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  await stroke(
    page,
    wobble.map(([x, y]) => [x, y + 80] as [number, number]),
  )
  await page.getByTestId('drawing-done').click()
  await expect(editor(page)).toHaveCount(0)
  await expect.poll(() => drawingOf(backend, 'pages/x9.md').svg).not.toBe(mine)
  const theirs = drawingOf(backend, 'pages/x9.md').svg
  backend.write(`assets/drawings/${first.name}`, mine)
  backend.write(copyName(first.name), theirs)
  await page.reload()

  // add its strokes: this drawing's stroke is not doubled, the other one comes in
  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-copy-banner')).toBeVisible()
  await page.getByTestId('drawing-copy-add').click()
  await expect(page.getByTestId('drawing-copy-banner')).toHaveCount(0)
  await page.getByTestId('drawing-done').click()
  await expect(editor(page)).toHaveCount(0)
  await expect.poll(() => strokeCount(backend.read(`assets/drawings/${first.name}`))).toBe(2)
  // the copy is dealt with: in the trash, not beside the drawing
  await expect.poll(() => backend.exists(copyName(first.name))).toBe(false)

  // keep this one: the copy goes at once
  backend.write(copyName(first.name), theirs)
  await page.reload()
  await page.getByTestId('drawing-image').click()
  await page.getByTestId('drawing-copy-keep').click()
  await expect(page.getByTestId('drawing-copy-banner')).toHaveCount(0)
  await expect.poll(() => backend.exists(copyName(first.name))).toBe(false)
  await page.getByTestId('drawing-cancel').click()

  // use that version: the drawing becomes the copy's
  backend.write(`assets/drawings/${first.name}`, mine)
  backend.write(copyName(first.name), theirs)
  await page.reload()
  await page.getByTestId('drawing-image').click()
  await page.getByTestId('drawing-copy-use').click()
  await page.getByTestId('drawing-done').click()
  await expect(editor(page)).toHaveCount(0)
  await expect.poll(() => strokeCount(backend.read(`assets/drawings/${first.name}`))).toBe(2)
  await expect.poll(() => backend.exists(copyName(first.name))).toBe(false)
})

test('DR-22: the drawing can be saved as SVG or PNG (named after the drawing)', async ({ backend, page }) => {
  backend.write('pages/x10.md', '- start\n')
  await slashDraw(page, backend, 'x10', 'start')
  await page.getByTestId('drawing-name').fill('Router sketch')
  await stroke(page, wobble)
  const svgDownload = page.waitForEvent('download')
  await more(page, 'drawing-save-svg')
  const svg = await svgDownload
  expect(svg.suggestedFilename()).toBe('Router sketch.svg')
  const fs = await import('node:fs')
  expect(fs.readFileSync((await svg.path())!, 'utf8')).toContain('<svg')

  const pngDownload = page.waitForEvent('download')
  await more(page, 'drawing-save-png')
  const png = await pngDownload
  expect(png.suggestedFilename()).toBe('Router sketch.png')
  const bytes = fs.readFileSync((await png.path())!)
  expect([...bytes.subarray(1, 4)].map((b) => String.fromCharCode(b)).join('')).toBe('PNG')
  // the longer side is about 2000 px
  const w = bytes.readUInt32BE(16)
  const h = bytes.readUInt32BE(20)
  expect(Math.max(w, h)).toBeGreaterThan(1500)
  expect(Math.max(w, h)).toBeLessThanOrEqual(4096)

  // Copy as a picture answers either way (the webview may not allow the clipboard)
  await more(page, 'drawing-copy-png')
  await expect(page.getByText(/Copied the drawing|cannot copy a picture/)).toBeVisible()
})

test('DR-24: a picture pasted into the canvas becomes part of the drawing', async ({ backend, page }) => {
  backend.write('pages/x12.md', '- start\n')
  await slashDraw(page, backend, 'x12', 'start')
  // the 1x1 PNG a screenshot tool would put on the clipboard, delivered as a paste event
  await page.evaluate(async () => {
    const png =
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='
    const bytes = Uint8Array.from(atob(png), (c) => c.charCodeAt(0))
    const dt = new DataTransfer()
    dt.items.add(new File([bytes], 'shot.png', { type: 'image/png' }))
    const target = document.querySelector(
      '[data-testid="drawing-editor"] .imageEditorContainer',
    ) as HTMLElement
    target.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }))
  })
  // the picture is read and added after the event
  await page.waitForTimeout(500)
  const { svg } = await finish(page, backend, 'x12')
  expect(svg).toContain('<image')
})
