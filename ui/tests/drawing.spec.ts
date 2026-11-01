import type { Page } from '@playwright/test'
import { expect, test } from './fixtures'

// DR-1 to DR-7: `/draw` opens the drawing editor, Done stores one SVG below assets/drawings and links it, a drawing
// in a block opens again for editing, and the handwriting is kept as it was written.

const SVG_LINK = /!\[\]\((?:\.\.\/)*assets\/drawings\/(draw-[0-9a-f]{8}\.svg)\)/

type Backend = {
  write: (p: string, t: string) => void
  open: (r: string) => Promise<void>
  read: (p: string) => string
  exists: (p: string) => boolean
}

/** Types `/draw` at the end of the block `text` and picks the command; the editor opens and has loaded. */
async function slashDraw(page: Page, backend: Backend, pageName: string, text: string) {
  await backend.open(`/page/${pageName}`)
  await page.getByRole('tree', { name: 'Blocks' }).getByText(text, { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /draw')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Drawing…', { timeout: 5000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('drawing-editor')).toBeVisible()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
}

/** One stroke with the mouse through `points` (offsets from the middle of the canvas); `hold` ms before it ends. */
async function stroke(page: Page, points: [number, number][], hold = 0) {
  const box = (await page.getByTestId('drawing-editor').locator('canvas').first().boundingBox())!
  const cx = box.x + box.width / 2
  const cy = box.y + box.height / 2 + 40
  await page.mouse.move(cx + points[0][0], cy + points[0][1])
  await page.mouse.down()
  for (const [x, y] of points.slice(1)) await page.mouse.move(cx + x, cy + y, { steps: 3 })
  if (hold) await page.waitForTimeout(hold)
  await page.mouse.up()
}

const wobble: [number, number][] = Array.from({ length: 24 }, (_, i) => [i * 12, Math.sin(i / 1.7) * 14])

/** The drawing the block links, as the text of its file. */
function drawingOf(backend: Backend, pageFile: string) {
  const name = SVG_LINK.exec(backend.read(pageFile))![1]
  return { name, svg: backend.read(`assets/drawings/${name}`) }
}

test('/draw: a stroke and Done store one SVG and link it where the command was', async ({
  backend,
  page,
}) => {
  backend.write('pages/d1.md', '- start\n')
  await slashDraw(page, backend, 'd1', 'start')
  // a labelled dialog that took the focus
  await expect(page.getByRole('dialog', { name: 'Drawing' })).toBeVisible()
  await stroke(page, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  // the tool settings (colour, width) are kept on this device
  await expect.poll(() => page.evaluate(() => localStorage.getItem('notatnik.drawing.tools'))).not.toBeNull()
  // the block is still being edited, with the link in it
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/d1.md'))
    .toMatch(/^- start !\[\]\(\.\.\/assets\/drawings\/draw-[0-9a-f]{8}\.svg\)\n$/)
  const { svg } = drawingOf(backend, 'pages/d1.md')
  expect(svg).toContain('<svg')
  expect(svg).toContain('<path')
  // black ink on a white sheet: it reads on a dark theme too
  expect(svg).toContain('stroke="#000000"')
  expect(svg).toContain('fill="#ffffff"')
  // and the picture shows in the block
  const img = page.getByTestId('drawing-image').locator('img')
  await expect(img).toBeVisible()
  await expect.poll(() => img.evaluate((i: HTMLImageElement) => i.complete && i.naturalWidth > 0)).toBe(true)
})

test('the handwriting stays as written: a held-still stroke is not turned into a line', async ({
  backend,
  page,
}) => {
  backend.write('pages/d2.md', '- start\n')
  await slashDraw(page, backend, 'd2', 'start')
  // a wobbly line, then the pen rests: with shape autocorrect on this becomes one straight segment
  await stroke(page, wobble, 1600)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d2.md')).toMatch(SVG_LINK)
  const d = /<path d="([^"]+)" fill="none"/.exec(drawingOf(backend, 'pages/d2.md').svg)![1]
  const curves = d.match(/[qQcC]/g) ?? []
  expect(curves.length).toBeGreaterThanOrEqual(5) // the wobbles are there
  expect(d).not.toMatch(/^M[\d.,-]+[lL][\d.,-]+$/) // not a single straight line
})

test('a drawing in a block opens again; the same file is overwritten and the picture is refreshed', async ({
  backend,
  page,
}) => {
  backend.write('pages/d3.md', '- start\n')
  await slashDraw(page, backend, 'd3', 'start')
  await stroke(page, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d3.md')).toMatch(SVG_LINK)
  const before = drawingOf(backend, 'pages/d3.md')
  const textBefore = backend.read('pages/d3.md')

  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-editor')).toBeVisible()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  // Done without a change stores nothing
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  expect(backend.read(`assets/drawings/${before.name}`)).toBe(before.svg)

  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  await stroke(
    page,
    wobble.map(([x, y]) => [x, y + 60] as [number, number]),
  )
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect.poll(() => drawingOf(backend, 'pages/d3.md').svg).not.toBe(before.svg)
  // the same file and the same link; the first stroke is still there (it was loaded as a stroke)
  const after = drawingOf(backend, 'pages/d3.md')
  expect(after.name).toBe(before.name)
  expect(backend.read('pages/d3.md')).toBe(textBefore)
  expect(after.svg.match(/stroke-width/g)?.length).toBe(2)
  // no second file
  expect(backend.exists('assets/drawings')).toBe(true)
  // the picture on the page is fetched again
  await expect(page.getByTestId('drawing-image').locator('img')).toHaveAttribute('src', /[?&]v=\d+/)
})

test('Cancel on an empty drawing closes at once and stores nothing; after drawing it asks first', async ({
  backend,
  page,
}) => {
  backend.write('pages/d4.md', '- start\n')
  await slashDraw(page, backend, 'd4', 'start')
  await page.getByTestId('drawing-cancel').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  expect(backend.exists('assets/drawings')).toBe(false)
  // the block is still being edited, without the command
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d4.md')).toBe('- start\n')

  // after a stroke Cancel (and Escape) ask; Keep goes on, Discard stores nothing
  await slashDraw(page, backend, 'd4', 'start')
  await stroke(page, wobble)
  await page.getByTestId('drawing-cancel').click()
  await expect(page.getByTestId('drawing-confirm')).toBeVisible()
  await page.getByTestId('drawing-keep').click()
  await expect(page.getByTestId('drawing-confirm')).toHaveCount(0)
  await expect(page.getByTestId('drawing-editor')).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('drawing-confirm')).toBeVisible()
  await page.getByTestId('drawing-discard').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  expect(backend.exists('assets/drawings')).toBe(false)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d4.md')).toBe('- start\n')
})

test('keys in the drawing editor do not reach the page behind it', async ({ backend, page }) => {
  backend.write('pages/d5.md', '- start\n')
  await slashDraw(page, backend, 'd5', 'start')
  // Alt+2 goes to the agenda anywhere else; here it is not ours
  await page.keyboard.press('Alt+2')
  await expect(page).toHaveURL(/#\/page\/d5/)
  await expect(page.getByTestId('drawing-editor')).toBeVisible()
})
