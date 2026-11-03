import type { Page } from '@playwright/test'
import { expect } from './fixtures'

// Helpers of the drawing tests (drawing.spec.ts, drawing-extras.spec.ts).

// a drawing written as a picture or as a link, with or without a name
export const SVG_LINK = /!?\[[^\]]*\]\((?:\.\.\/)*assets\/drawings\/(draw-[0-9a-f]{8}\.svg)\)/

export type Backend = {
  write: (p: string, t: string) => void
  open: (r: string) => Promise<void>
  read: (p: string) => string
  exists: (p: string) => boolean
}

/** Types `/draw` at the end of the block `text` and picks the command; the editor opens and has loaded. */
export async function slashDraw(page: Page, backend: Backend, pageName: string, text: string) {
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
export async function stroke(page: Page, points: [number, number][], hold = 0, steps = 3) {
  const box = (await page.getByTestId('drawing-editor').locator('canvas').first().boundingBox())!
  const cx = box.x + box.width / 2
  const cy = box.y + box.height / 2 + 40
  await page.mouse.move(cx + points[0][0], cy + points[0][1])
  await page.mouse.down()
  for (const [x, y] of points.slice(1)) await page.mouse.move(cx + x, cy + y, { steps })
  if (hold) await page.waitForTimeout(hold)
  await page.mouse.up()
}

export const wobble: [number, number][] = Array.from({ length: 24 }, (_, i) => [
  i * 12,
  Math.sin(i / 1.7) * 14,
])

/** The sheet the drawing is on (white, with its grid, DR-1) is a group of its own in the file: it is not a stroke. */
export const withoutSheet = (svg: string) =>
  svg.replace(/<g class="js-draw-image-background[^"]*">.*?<\/g>/g, '')

/** The drawing the block links, as the text of its file. */
export function drawingOf(backend: Backend, pageFile: string) {
  const name = SVG_LINK.exec(backend.read(pageFile))![1]
  return { name, svg: backend.read(`assets/drawings/${name}`) }
}

// DR-2: the pen draws, a finger moves the canvas once a stylus has been seen (a resting hand does not draw). The
// pointer events are made by hand: a touch screen and a stylus are what a test cannot press.
export async function pointerStroke(
  page: Page,
  pointerType: 'pen' | 'touch',
  id: number,
  points: [number, number][],
) {
  const canvas = page.getByTestId('drawing-editor').locator('canvas').first()
  const box = (await canvas.boundingBox())!
  const at = (p: [number, number], extra: object) => ({
    pointerType,
    pointerId: id,
    isPrimary: true,
    pressure: 0.5,
    clientX: box.x + box.width / 2 + p[0],
    clientY: box.y + box.height / 2 + p[1] + 40,
    ...extra,
  })
  await canvas.dispatchEvent('pointerdown', at(points[0], { buttons: 1 }))
  for (const p of points.slice(1)) await canvas.dispatchEvent('pointermove', at(p, { buttons: 1 }))
  await canvas.dispatchEvent('pointerup', at(points[points.length - 1], { buttons: 0 }))
}
