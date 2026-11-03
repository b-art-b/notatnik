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

// DR-1 to DR-7: `/draw` opens the drawing editor, Done stores one SVG below assets/drawings and links it, a drawing
// in a block opens again for editing, and the handwriting is kept as it was written.

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
  const d = /<path d="([^"]+)" fill="none"/.exec(withoutSheet(drawingOf(backend, 'pages/d2.md').svg))![1]
  const curves = d.match(/[qQcC]/g) ?? []
  // nothing is fitted away: the 24 moves of the mouse (3 steps each) are all in the line, one piece per point
  expect(curves.length).toBeGreaterThanOrEqual(60)
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
  expect(withoutSheet(after.svg).match(/stroke-width/g)?.length).toBe(2)
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

// A drawing has a name (the label of its reference), is shown as a picture or only as a link, and can be deleted.
test('a drawing can be named and written as a link; the link opens it with its name', async ({
  backend,
  page,
}) => {
  backend.write('pages/d6.md', '- start\n')
  await slashDraw(page, backend, 'd6', 'start')
  await page.getByTestId('drawing-name').fill('Router sketch')
  await page.getByRole('radio', { name: 'Link' }).check()
  await stroke(page, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/d6.md'))
    .toMatch(/^- start \[Router sketch\]\(\.\.\/assets\/drawings\/draw-[0-9a-f]{8}\.svg\)\n$/)
  // a link, not a picture
  const link = page.getByTestId('drawing-link')
  await expect(link).toContainText('Router sketch')
  await expect(page.getByTestId('drawing-image')).toHaveCount(0)

  // it opens with its name and form
  await link.click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  await expect(page.getByTestId('drawing-name')).toHaveValue('Router sketch')
  await expect(page.getByRole('radio', { name: 'Link' })).toBeChecked()
})

test('the name and the form change in the note, the file stays as it is', async ({ backend, page }) => {
  backend.write('pages/d7.md', '- start\n')
  await slashDraw(page, backend, 'd7', 'start')
  await page.getByTestId('drawing-name').fill('First name')
  await page.getByRole('radio', { name: 'Link' }).check()
  await stroke(page, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d7.md')).toMatch(/\[First name\]/)
  const before = drawingOf(backend, 'pages/d7.md')

  // renamed and shown as a picture
  await page.getByTestId('drawing-link').click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  await page.getByTestId('drawing-name').fill('Better name')
  await page.getByRole('radio', { name: 'Picture' }).check()
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect
    .poll(() => backend.read('pages/d7.md'))
    .toBe(`- start ![Better name](../assets/drawings/${before.name})\n`)
  // the drawing itself was not rewritten
  expect(backend.read(`assets/drawings/${before.name}`)).toBe(before.svg)
  const img = page.getByTestId('drawing-image').locator('img')
  await expect(img).toHaveAttribute('alt', 'Better name')

  // and back to a link without a name
  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  await page.getByTestId('drawing-name').fill('')
  await page.getByRole('radio', { name: 'Link' }).check()
  await page.getByTestId('drawing-done').click()
  await expect
    .poll(() => backend.read('pages/d7.md'))
    .toBe(`- start [Drawing](../assets/drawings/${before.name})\n`)
})

test('a drawing is deleted after asking: out of the note, its file to the trash', async ({
  backend,
  page,
}) => {
  backend.write('pages/d8.md', '- start\n')
  await slashDraw(page, backend, 'd8', 'start')
  await stroke(page, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d8.md')).toMatch(SVG_LINK)
  const { name } = drawingOf(backend, 'pages/d8.md')
  expect(backend.exists(`assets/drawings/${name}`)).toBe(true)

  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  // asked first; Keep it changes nothing
  await page.getByTestId('drawing-delete').click()
  await expect(page.getByTestId('drawing-delete-confirm')).toBeVisible()
  await page.getByTestId('drawing-delete-keep').click()
  await expect(page.getByTestId('drawing-delete-confirm')).toHaveCount(0)
  await expect(page.getByTestId('drawing-editor')).toBeVisible()
  expect(backend.exists(`assets/drawings/${name}`)).toBe(true)

  await page.getByTestId('drawing-delete').click()
  await page.getByTestId('drawing-delete-yes').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect.poll(() => backend.read('pages/d8.md')).toBe('- start\n')
  expect(backend.exists(`assets/drawings/${name}`)).toBe(false)
  await expect(page.getByTestId('drawing-image')).toHaveCount(0)
})

test('a drawing that another page links too stays as a file when it is deleted here', async ({
  backend,
  page,
}) => {
  backend.write('pages/d9.md', '- start\n')
  await slashDraw(page, backend, 'd9', 'start')
  await stroke(page, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d9.md')).toMatch(SVG_LINK)
  const { name } = drawingOf(backend, 'pages/d9.md')
  backend.write('pages/d9b.md', `- same drawing ![](../assets/drawings/${name})\n`)

  await page.getByTestId('drawing-image').click()
  await expect(page.getByTestId('drawing-done')).toBeEnabled()
  await page.getByTestId('drawing-delete').click()
  await page.getByTestId('drawing-delete-yes').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect.poll(() => backend.read('pages/d9.md')).toBe('- start\n')
  // still there for the other page
  expect(backend.exists(`assets/drawings/${name}`)).toBe(true)
  await expect(page.getByText('Another page links this drawing too')).toBeVisible()
})

const strokesOf = (svg: string) => (withoutSheet(svg).match(/fill="none"/g) ?? []).length

test('without a stylus a finger draws', async ({ backend, page }) => {
  backend.write('pages/d10.md', '- start\n')
  await slashDraw(page, backend, 'd10', 'start')
  await pointerStroke(page, 'touch', 21, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d10.md')).toMatch(SVG_LINK)
  expect(strokesOf(drawingOf(backend, 'pages/d10.md').svg)).toBe(1)
})

test('once a stylus has been seen a finger moves the canvas and does not draw', async ({ backend, page }) => {
  backend.write('pages/d11.md', '- start\n')
  await slashDraw(page, backend, 'd11', 'start')
  await pointerStroke(page, 'pen', 31, wobble)
  // the hand rests on the glass and moves: no second stroke
  await pointerStroke(
    page,
    'touch',
    32,
    wobble.map(([x, y]) => [x, y + 80] as [number, number]),
  )
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d11.md')).toMatch(SVG_LINK)
  expect(strokesOf(drawingOf(backend, 'pages/d11.md').svg)).toBe(1)
  // and it is remembered: the next editor starts that way
  expect(await page.evaluate(() => localStorage.getItem('notatnik.drawing.stylus'))).toBe('1')
})

// DR-12: "Make space" moves everything below a line, in one undo step.

/** The height of the first point of each stroke in a saved drawing, top to bottom as they were drawn. */
function strokeHeights(svg: string): number[] {
  return [
    ...withoutSheet(svg).matchAll(
      /<path(?![^>]*js-draw-image-background)[^>]*\sd="[Mm]\s*(-?[\d.]+)[ ,]+(-?[\d.]+)/g,
    ),
  ].map((m) => Number(m[2]))
}

/** Three strokes (C above A above the line, B below it), then the Make space tool dragged from the line by `drag`
 * (down when positive), with the mouse or with a pen pointer made by hand. */
async function makeSpace(page: Page, drag: number, how: 'mouse' | 'pen' = 'mouse') {
  const line = (r: number): [number, number][] => [
    [0, r],
    [140, r],
  ]
  await stroke(page, line(-120)) // C
  await stroke(page, line(-60)) // A
  await stroke(page, line(60)) // B
  const editor = page.getByTestId('drawing-editor')
  await editor.getByRole('button', { name: 'Make space' }).click()
  await expect(page.getByTestId('drawing-hint')).toContainText('Make space')
  const shown = page.getByTestId('space-line')
  if (how === 'pen') {
    await pointerStroke(page, 'pen', 41, [
      [60, 0],
      [60, drag / 2],
      [60, drag],
    ])
    await expect(shown).toHaveCount(0)
    return
  }
  const box = (await editor.locator('canvas').first().boundingBox())!
  const x = box.x + box.width / 2 + 60
  const y = box.y + box.height / 2 + 40
  await page.mouse.move(x, y)
  await page.mouse.down()
  // while dragging the line is shown, as a dashed line that is really visible
  await page.mouse.move(x, y + drag / 2, { steps: 4 })
  await expect(shown).toBeVisible()
  expect(await shown.evaluate((e) => getComputedStyle(e).borderTopStyle)).toBe('dashed')
  await page.mouse.move(x, y + drag, { steps: 4 })
  await page.mouse.up()
  await expect(shown).toHaveCount(0)
}

/** Done, and the heights of the three strokes C, A, B as they were saved. */
async function savedHeights(page: Page, backend: Backend, pageName: string) {
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read(`pages/${pageName}.md`)).toMatch(SVG_LINK)
  return strokeHeights(drawingOf(backend, `pages/${pageName}.md`).svg)
}

const undo = (page: Page) => page.getByTestId('drawing-editor').getByRole('button', { name: 'Undo' }).click()
const redo = (page: Page) => page.getByTestId('drawing-editor').getByRole('button', { name: 'Redo' }).click()

test('Make space moves what is below the line and nothing above it', async ({ backend, page }) => {
  backend.write('pages/d12.md', '- start\n')
  await slashDraw(page, backend, 'd12', 'start')
  await makeSpace(page, 100)
  const [c, a, b] = await savedHeights(page, backend, 'd12')
  // C and A are still 60 apart; B was 120 below A and is now 100 further down
  expect(a - c).toBeCloseTo(60, 0)
  expect(b - a).toBeCloseTo(220, 0)
})

test('Make space: one undo moves everything back, one redo moves it again', async ({ backend, page }) => {
  backend.write('pages/d13.md', '- start\n')
  await slashDraw(page, backend, 'd13', 'start')
  await makeSpace(page, 100)
  await undo(page)
  // the whole drag was one step: the three strokes are drawn again as they were, not left half way
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d13.md')).toMatch(SVG_LINK)
  const [c, a, b] = strokeHeights(drawingOf(backend, 'pages/d13.md').svg)
  expect(a - c).toBeCloseTo(60, 0)
  expect(b - a).toBeCloseTo(120, 0)

  backend.write('pages/d14.md', '- start\n')
  await slashDraw(page, backend, 'd14', 'start')
  await makeSpace(page, 100)
  await undo(page)
  await redo(page)
  const [c2, a2, b2] = await savedHeights(page, backend, 'd14')
  expect(a2 - c2).toBeCloseTo(60, 0)
  expect(b2 - a2).toBeCloseTo(220, 0)
})

test('Make space: dragging up never takes the lower part above the line', async ({ backend, page }) => {
  backend.write('pages/d15.md', '- start\n')
  await slashDraw(page, backend, 'd15', 'start')
  await makeSpace(page, -500)
  const [c, a, b] = await savedHeights(page, backend, 'd15')
  expect(a - c).toBeCloseTo(60, 0)
  // B came up to the line (its top edge is 2 above its centre) and no further: it is still below A, by about 60
  expect(b - a).toBeGreaterThan(55)
  expect(b - a).toBeLessThan(70)
})

test('Make space sideways moves what is to the right of a vertical line, and nothing to the left', async ({
  backend,
  page,
}) => {
  backend.write('pages/d18.md', '- start\n')
  await slashDraw(page, backend, 'd18', 'start')
  // three vertical strokes: C and A left of the line, B right of it
  const bar = (x: number): [number, number][] => [
    [x, -40],
    [x, 80],
  ]
  await stroke(page, bar(-120)) // C
  await stroke(page, bar(-60)) // A
  await stroke(page, bar(60)) // B
  const editor = page.getByTestId('drawing-editor')
  await editor.getByRole('button', { name: 'Make space' }).click()
  const box = (await editor.locator('canvas').first().boundingBox())!
  const x = box.x + box.width / 2
  const y = box.y + box.height / 2 + 40
  const shown = page.getByTestId('space-line')
  await page.mouse.move(x, y)
  await page.mouse.down()
  await page.mouse.move(x + 50, y, { steps: 4 })
  // a vertical dashed line, as tall as the page
  await expect(shown).toBeVisible()
  expect(await shown.evaluate((e) => getComputedStyle(e).borderLeftStyle)).toBe('dashed')
  await page.mouse.move(x + 100, y, { steps: 4 })
  await page.mouse.up()
  await expect(shown).toHaveCount(0)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d18.md')).toMatch(SVG_LINK)
  const xs = [
    ...withoutSheet(drawingOf(backend, 'pages/d18.md').svg).matchAll(
      /<path(?![^>]*js-draw-image-background)[^>]*\sd="[Mm]\s*(-?[\d.]+)[ ,]+(-?[\d.]+)/g,
    ),
  ].map((m) => Number(m[1]))
  const [c, a, b] = xs
  // C and A are still 60 apart; B was 120 right of A and is now 100 further right
  expect(a - c).toBeCloseTo(60, 0)
  expect(b - a).toBeCloseTo(220, 0)
})

test('Make space works with a pen pointer', async ({ backend, page }) => {
  backend.write('pages/d16.md', '- start\n')
  await slashDraw(page, backend, 'd16', 'start')
  await makeSpace(page, 100, 'pen')
  const [c, a, b] = await savedHeights(page, backend, 'd16')
  expect(a - c).toBeCloseTo(60, 0)
  expect(b - a).toBeCloseTo(220, 0)
})

test('Make space is switched off by picking a pen, and the pen draws again', async ({ backend, page }) => {
  backend.write('pages/d17.md', '- start\n')
  await slashDraw(page, backend, 'd17', 'start')
  const editor = page.getByTestId('drawing-editor')
  await editor.getByRole('button', { name: 'Make space' }).click()
  await expect(page.getByTestId('drawing-hint')).toContainText('Make space')
  await editor
    .getByRole('button', { name: /^Handwriting|^Pen/ })
    .first()
    .click()
  await expect(page.getByTestId('drawing-hint')).not.toContainText('Make space')
  await stroke(page, wobble)
  await page.getByTestId('drawing-done').click()
  await expect(page.getByTestId('drawing-editor')).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/d17.md')).toMatch(SVG_LINK)
  expect(strokesOf(drawingOf(backend, 'pages/d17.md').svg)).toBe(1)
})
