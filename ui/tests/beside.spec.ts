import { expect, test } from './fixtures'
import type { Page } from '@playwright/test'

// Two graphs side by side in one window (MG-1, MG-6): the second graph is the whole app in a frame beside the first.
const pane = (page: Page) => page.getByTestId('beside-pane')
const frame = (page: Page) => page.frameLocator('[data-testid=beside-frame]')

test('a second graph shows beside the first, each with its own pages, chip and color', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Tale.md', '- tale of beta\n')
  backend.write('pages/Saga.md', '- saga of the first\n')
  await backend.open('/page/Saga')
  await expect(pane(page)).toBeVisible()
  await expect(pane(page).getByTestId('beside-name')).toContainText('Beta')

  // the frame is a whole app: it shows the second graph's pages, not the first one's
  const inside = frame(page)
  await expect(inside.getByRole('main')).toBeVisible()
  await expect(inside.getByTestId('graph-chip')).toContainText('Beta')
  await page.locator('[data-testid=beside-frame]').evaluate((f) => {
    ;(f as HTMLIFrameElement).contentWindow!.location.hash = '#/page/Tale'
  })
  await expect(inside.getByText('tale of beta')).toBeVisible()
  await expect(inside.getByText('saga of the first')).toHaveCount(0)
  await expect(page.getByRole('main').getByText('saga of the first')).toBeVisible()
  await expect(page.getByRole('main').getByTestId('graph-chip')).toContainText('graph')

  // each pane has the color of its own graph (the root attribute of each page)
  const color = (await beta.call<{ color: number }>('graph_info')).color
  const frameColor = await page
    .locator('[data-testid=beside-frame]')
    .evaluate((f) => (f as HTMLIFrameElement).contentDocument!.documentElement.dataset.graphColor)
  expect(frameColor).toBe(String(color))
  // editing in the frame writes to the second graph only
  await inside.getByText('tale of beta').click()
  await page.keyboard.press('End')
  await page.keyboard.type(' and more')
  await page.keyboard.press('Escape')
  await expect.poll(() => beta.read('pages/Tale.md')).toContain('tale of beta and more')
  expect(backend.read('pages/Saga.md')).toBe('- saga of the first\n')
  expect(backend.exists('pages/Tale.md')).toBe(false)
})

test('the sidebar of the frame starts closed and the first pane keeps its own', async ({ backend, page }) => {
  await backend.beside('Beta')
  await backend.open('/journals')
  await expect(page.getByRole('complementary', { name: 'Sidebar' })).toBeVisible()
  await expect(frame(page).getByRole('complementary', { name: 'Sidebar' })).toBeHidden()
  await frame(page).getByTestId('left-toggle').click()
  await expect(frame(page).getByRole('complementary', { name: 'Sidebar' })).toBeVisible()
  // opening the frame's sidebar did not change the first pane's setting
  await page.reload()
  await expect(page.getByRole('complementary', { name: 'Sidebar' })).toBeVisible()
  await expect(frame(page).getByRole('complementary', { name: 'Sidebar' })).toBeHidden()
})

test('a graph from the history opens beside the first from the graph menu, and the pane closes again', async ({
  backend,
  page,
}) => {
  // a second graph in the history of this device: opened once, then the first graph again
  const other = `${backend.dir}-other`
  await backend.call('open_graph', { path: other, create: true })
  await backend.call('open_graph', { path: backend.dir, create: false })
  await backend.open('/journals')
  await expect(pane(page)).toHaveCount(0)

  await page.getByTestId('graph-switcher').click()
  await page.getByTestId('graph-beside').click()
  await expect(pane(page)).toBeVisible()
  await expect(pane(page).getByTestId('beside-name')).toContainText('-other')
  await expect(frame(page).getByRole('main')).toBeVisible()
  // the app's width is shared: the first pane did not run out of its window
  const main = await page.getByRole('main').boundingBox()
  const side = await pane(page).boundingBox()
  expect(main!.x + main!.width).toBeLessThanOrEqual(side!.x + 1)
  expect(side!.x + side!.width).toBeLessThanOrEqual(page.viewportSize()!.width + 1)

  // the same graph cannot be in both panes
  const slots = await backend.call<{ slot: string; name: string }[]>('slots')
  expect(slots.map((s) => s.slot)).toEqual(['', '1'])

  // the first pane cannot switch to the graph the other pane shows
  await page.getByTestId('graph-switcher').click()
  await page
    .getByRole('menuitem', { name: /-other/ })
    .first()
    .click()
  await expect(page.locator('.toast.error')).toContainText('already open')
  await expect(page.getByTestId('graph-switcher')).not.toContainText('-other')

  await pane(page).getByTestId('beside-close').click()
  await expect(pane(page)).toHaveCount(0)
  const left = await backend.call<{ slot: string }[]>('slots')
  expect(left.map((s) => s.slot)).toEqual([''])
})

test('a page dragged from one pane to the other opens the dialog there, and the copy lands in that graph', async ({
  backend,
  page,
  browserName,
}) => {
  // Playwright's WebKit cannot start a drag that ends in a frame (a drag inside one frame works); the desktop app
  // needs a look by hand, see docs/testing.md
  test.skip(browserName === 'webkit', 'a drag across frames is not simulated in WebKit')
  const beta = await backend.beside('Beta')
  backend.write('pages/Dragged.md', '- drag me over\n')
  await backend.open('/page/Dragged')
  await expect(frame(page).getByRole('main')).toBeVisible()
  await page
    .getByRole('main')
    .getByRole('heading', { name: 'Dragged' })
    .locator('.title')
    .dragTo(frame(page).getByRole('main'))
  const d = frame(page).getByTestId('transfer-dialog')
  await expect(d).toBeVisible()
  await expect(d.getByTestId('transfer-source')).toHaveText('graph')
  await expect(d.getByTestId('transfer-plan')).toContainText('New page Dragged')
  await d.getByTestId('transfer-go').click()
  await expect.poll(() => beta.exists('pages/Dragged.md')).toBe(true)
  expect(backend.exists('pages/Dragged.md')).toBe(true)
})

test('a block dragged to the other pane is added to the page shown there, and a move empties the first pane', async ({
  backend,
  page,
  browserName,
}) => {
  // Playwright's WebKit cannot start a drag that ends in a frame (a drag inside one frame works); the desktop app
  // needs a look by hand, see docs/testing.md
  test.skip(browserName === 'webkit', 'a drag across frames is not simulated in WebKit')
  const beta = await backend.beside('Beta')
  beta.write('pages/Inbox.md', '- already in beta\n')
  backend.write('pages/Notes.md', '- stays\n- travels along\n  - with its child\n')
  await backend.open('/page/Notes')
  await page.locator('[data-testid=beside-frame]').evaluate((f) => {
    ;(f as HTMLIFrameElement).contentWindow!.location.hash = '#/page/Inbox'
  })
  await expect(frame(page).getByText('already in beta')).toBeVisible()

  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).nth(1).dragTo(frame(page).getByRole('main'))
  const d = frame(page).getByTestId('transfer-dialog')
  await expect(d.getByTestId('transfer-page-name')).toHaveValue('Inbox')
  await d.getByTestId('transfer-move').check()
  await expect(d.getByTestId('transfer-plan')).toContainText('Added to Inbox')
  await d.getByTestId('transfer-go').click()
  await expect
    .poll(() => beta.read('pages/Inbox.md'))
    .toBe('- already in beta\n- travels along\n  - with its child\n')
  await expect.poll(() => backend.read('pages/Notes.md')).toBe('- stays\n')
  // the pane the block left shows it gone, and the pane it went to shows it arrived
  await expect(blocks.getByText('travels along')).toHaveCount(0)
  await expect(frame(page).getByText('travels along')).toBeVisible()
})
