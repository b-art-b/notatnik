import type { Page } from '@playwright/test'
import { expect, test } from './fixtures'
import type { Backend } from './drawing-helpers'

// DG-1 to DG-4, DG-8, DG-10 (X3): `/diagram` opens the Excalidraw editor in a dialog; Done stores one SVG with the scene
// below assets/drawings/ and links it, a diagram in a block opens again for editing, and a draft is kept.

const LINK = /!?\[[^\]]*\]\((?:\.\.\/)*assets\/drawings\/(diagram-[0-9a-f]{8}\.svg)\)/
/** The shapes of an exported diagram: one group each (the white sheet behind them is a <rect>). */
const shapes = (svg: string) => (svg.match(/<g stroke-linecap/g) ?? []).length
const SCENE = 'payload-type:application/vnd.excalidraw+json'

/** Types `/diagram` at the end of the block `text` and picks the command; the editor has opened and loaded. */
async function slashDiagram(page: Page, backend: Backend, pageName: string, text: string) {
  await backend.open(`/page/${pageName}`)
  await page.getByRole('tree', { name: 'Blocks' }).getByText(text, { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /diagram')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Diagram…', { timeout: 5000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('diagram-editor')).toBeVisible()
  await expect(page.getByTestId('diagram-done')).toBeEnabled({ timeout: 20_000 })
  await expect(page.locator('.excalidraw canvas.interactive')).toBeVisible()
}

/** A rectangle with the mouse, to the right of Excalidraw's own panel; `dx` moves it for a second one. */
async function rectangle(page: Page, dx = 0) {
  await page.getByTestId('toolbar-rectangle').click({ force: true })
  const box = (await page.locator('.excalidraw canvas.interactive').boundingBox())!
  await page.mouse.move(box.x + 420 + dx, box.y + 280)
  await page.mouse.down()
  await page.waitForTimeout(80)
  await page.mouse.move(box.x + 500 + dx, box.y + 320, { steps: 8 })
  await page.mouse.move(box.x + 560 + dx, box.y + 380, { steps: 8 })
  await page.mouse.up()
  await page.waitForTimeout(150)
}

const draftCount = (page: Page) =>
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

test('/diagram: a shape and Done store one SVG with the scene and link it where the command was', async ({
  backend,
  page,
}) => {
  backend.write('pages/g1.md', '- start\n')
  await slashDiagram(page, backend, 'g1', 'start')
  // a labelled dialog; the focus is inside it, and Escape would not reach the block behind
  await expect(page.getByRole('dialog', { name: 'Diagram' })).toBeVisible()
  expect(await page.evaluate(() => !!document.activeElement?.closest('[data-testid="diagram-editor"]'))).toBe(
    true,
  )
  await rectangle(page)
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  // the block is still being edited, with the link in it
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await expect.poll(() => backend.read('pages/g1.md')).toMatch(LINK)
  const m = LINK.exec(backend.read('pages/g1.md'))!
  const svg = backend.read(`assets/drawings/${m![1]}`)
  expect(svg.startsWith('<svg')).toBe(true)
  expect(svg).toContain(SCENE)
  expect(shapes(svg)).toBe(1)
  // no other file was made
  expect(backend.exists(`assets/drawings/${m![1]}`)).toBe(true)
})

test('Cancel on an empty diagram closes at once and stores nothing; after drawing it asks first', async ({
  backend,
  page,
}) => {
  backend.write('pages/g2.md', '- start\n')
  await slashDiagram(page, backend, 'g2', 'start')
  await page.getByTestId('diagram-cancel').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  expect(LINK.test(backend.read('pages/g2.md'))).toBe(false)

  await slashDiagram(page, backend, 'g2', 'start')
  await rectangle(page)
  await page.getByTestId('diagram-cancel').click()
  await expect(page.getByTestId('diagram-confirm')).toBeVisible()
  await page.getByTestId('diagram-keep').click()
  await expect(page.getByTestId('diagram-confirm')).toHaveCount(0)
  await expect(page.getByTestId('diagram-editor')).toBeVisible()
  await page.getByTestId('diagram-cancel').click()
  await page.getByTestId('diagram-discard').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  expect(LINK.test(backend.read('pages/g2.md'))).toBe(false)
})

test('a diagram in a block opens again: nothing changed stores nothing, a new shape overwrites the same file', async ({
  backend,
  page,
}) => {
  backend.write('pages/g3.md', '- start\n')
  await slashDiagram(page, backend, 'g3', 'start')
  await rectangle(page)
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  await expect.poll(() => backend.read('pages/g3.md')).toMatch(LINK)
  await page.keyboard.press('Escape')
  const before = backend.read('pages/g3.md')
  const file = `assets/drawings/${LINK.exec(before)![1]}`
  const svg1 = backend.read(file)

  // open, change nothing, Done: the file is as it was
  await page.getByRole('tree', { name: 'Blocks' }).locator('img').first().click()
  await expect(page.getByTestId('diagram-editor')).toBeVisible()
  await expect(page.getByTestId('diagram-done')).toBeEnabled({ timeout: 20_000 })
  await expect(page.locator('.excalidraw canvas.interactive')).toBeVisible()
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  expect(backend.read(file)).toBe(svg1)

  // open again, add a second shape, Done: the same file, now different; the block text is unchanged
  await page.getByRole('tree', { name: 'Blocks' }).locator('img').first().click()
  await expect(page.getByTestId('diagram-done')).toBeEnabled({ timeout: 20_000 })
  await expect(page.locator('.excalidraw canvas.interactive')).toBeVisible()
  await rectangle(page, 140)
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  const svg2 = backend.read(file)
  expect(svg2).not.toBe(svg1)
  expect(shapes(svg1)).toBe(1)
  expect(shapes(svg2)).toBe(2)
  expect(backend.read('pages/g3.md')).toBe(before)
})

test('name, form and Delete work as for a drawing (DG-4)', async ({ backend, page }) => {
  backend.write('pages/g4.md', '- start\n')
  await slashDiagram(page, backend, 'g4', 'start')
  await page.getByTestId('diagram-name').fill('Rack plan')
  await rectangle(page)
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  await expect.poll(() => backend.read('pages/g4.md')).toMatch(/!\[Rack plan\]\(/)
  await page.keyboard.press('Escape')
  const file = `assets/drawings/${LINK.exec(backend.read('pages/g4.md'))![1]}`

  // renamed and written as a link: only the reference changes, the file stays
  const svg = backend.read(file)
  await page.getByRole('tree', { name: 'Blocks' }).locator('img').first().click()
  await expect(page.getByTestId('diagram-done')).toBeEnabled({ timeout: 20_000 })
  await expect(page.getByTestId('diagram-name')).toHaveValue('Rack plan')
  await page.getByTestId('diagram-name').fill('Rack v2')
  await page.getByRole('radio', { name: 'Link' }).check()
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  expect(backend.read('pages/g4.md')).toMatch(/- start \[Rack v2\]\(/)
  expect(backend.read(file)).toBe(svg)

  // Delete asks; then the reference and the file are gone
  await page.getByTestId('drawing-link').first().click()
  await expect(page.getByTestId('diagram-done')).toBeEnabled({ timeout: 20_000 })
  await page.getByTestId('diagram-delete').click()
  await page.getByTestId('diagram-delete-keep').click()
  expect(backend.exists(file)).toBe(true)
  await page.getByTestId('diagram-delete').click()
  await page.getByTestId('diagram-delete-yes').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  expect(backend.exists(file)).toBe(false)
  expect(LINK.test(backend.read('pages/g4.md'))).toBe(false)
})

test('DG-8: a diagram that was not saved is kept, offered again after a restart, and gone once saved', async ({
  backend,
  page,
}) => {
  backend.write('pages/g5.md', '- start\n')
  await slashDiagram(page, backend, 'g5', 'start')
  await rectangle(page)
  await expect.poll(() => draftCount(page), { timeout: 15_000 }).toBe(1)
  await page.reload()
  await slashDiagram(page, backend, 'g5', 'start')
  await expect(page.getByTestId('diagram-draft-banner')).toBeVisible()
  await page.getByTestId('diagram-draft-restore').click()
  await expect(page.getByTestId('diagram-draft-banner')).toHaveCount(0)
  await expect(page.locator('.excalidraw canvas.interactive')).toBeVisible()
  await expect(page.getByTestId('diagram-done')).toBeEnabled({ timeout: 20_000 })
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  await expect.poll(() => backend.read('pages/g5.md')).toMatch(LINK)
  const m = LINK.exec(backend.read('pages/g5.md'))!
  expect(shapes(backend.read(`assets/drawings/${m[1]}`))).toBe(1)
  await expect.poll(() => draftCount(page)).toBe(0)
})

test('DG-10: the editor follows the dark theme, the stored picture is on white', async ({
  backend,
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'dark' })
  backend.write('pages/g6.md', '- start\n')
  await slashDiagram(page, backend, 'g6', 'start')
  await expect(page.locator('.excalidraw.theme--dark')).toBeVisible()
  await rectangle(page)
  await page.getByTestId('diagram-done').click()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  await expect.poll(() => backend.read('pages/g6.md')).toMatch(LINK)
  const svg = backend.read(`assets/drawings/${LINK.exec(backend.read('pages/g6.md'))![1]}`)
  expect(svg).toMatch(/fill="#ffffff"/i)
  expect(svg).not.toMatch(/filter:\s*invert|theme--dark/)
})

test('a drawing still opens the handwriting editor and a diagram the Excalidraw one', async ({
  backend,
  page,
}) => {
  backend.write(
    'assets/drawings/draw-0a1b2c3d.svg',
    '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80" viewBox="0 0 120 80"><path d="M0 0L50 50" stroke="black"/></svg>',
  )
  backend.write('pages/g7.md', '- ![](../assets/drawings/draw-0a1b2c3d.svg)\n')
  await backend.open('/page/g7')
  await page.getByRole('tree', { name: 'Blocks' }).locator('img').first().click()
  await expect(page.getByTestId('drawing-editor')).toBeVisible()
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
})

test('Escape closes an empty diagram and does not reach the block behind it', async ({ backend, page }) => {
  backend.write('pages/g8.md', '- start\n')
  await slashDiagram(page, backend, 'g8', 'start')
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('diagram-editor')).toHaveCount(0)
  // one Escape was used by the dialog: the block is still being edited
  await expect(page.getByTestId('block-editor')).toBeFocused()
})
