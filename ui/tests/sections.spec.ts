import { expect, test } from './fixtures'
import type { Page } from '@playwright/test'

// Two graphs in the sidebar (MG-6): a section per graph, the page area shows the one that was opened last.
const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Sidebar' })
const section = (page: Page, slot: string) =>
  sidebar(page).locator(`[data-testid=graph-section][data-slot="${slot}"]`)
const main = (page: Page) => page.getByRole('main')

/** The computed color the palette gives entry `n`. */
const paletteColor = (page: Page, n: number) =>
  page.evaluate((i) => {
    const probe = document.createElement('div')
    probe.style.background = `var(--graph-${i})`
    document.body.append(probe)
    const c = getComputedStyle(probe).backgroundColor
    probe.remove()
    return c
  }, n)

test('two sections: each lists its own pages, and a click in the second makes its graph the one on the page area', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Tale.md', '- tale of beta\n')
  backend.write('pages/Saga.md', '- saga of the first\n')
  await backend.open('/page/Saga')
  await expect(section(page, '')).toBeVisible()
  await expect(section(page, '1')).toBeVisible()
  // the first graph is active and open, the second is folded to its header
  await expect(section(page, '')).toHaveClass(/active/)
  await expect(section(page, '1')).not.toHaveClass(/active/)
  await expect(section(page, '1').getByRole('navigation')).toHaveCount(0)
  await expect(section(page, '1').getByTestId('graph-switcher')).toContainText('Beta')

  // each bar has the color of its own graph
  const c0 = (await backend.call<{ color: number }>('graph_info')).color
  const c1 = (await beta.call<{ color: number }>('graph_info')).color
  await expect(section(page, '')).toHaveCSS('border-top-color', await paletteColor(page, c0))
  await expect(section(page, '1')).toHaveCSS('border-top-color', await paletteColor(page, c1))

  await section(page, '1').getByTestId('section-fold').click()
  await expect(section(page, '1').getByRole('tree', { name: 'Namespaces' }).getByText('Tale')).toBeVisible()
  await expect(section(page, '1').getByRole('tree', { name: 'Namespaces' }).getByText('Saga')).toHaveCount(0)
  await expect(section(page, '').getByRole('tree', { name: 'Namespaces' }).getByText('Saga')).toBeVisible()
  await expect(section(page, '').getByRole('tree', { name: 'Namespaces' }).getByText('Tale')).toHaveCount(0)

  // a page of the second graph: the page area switches, the section is marked, the hash says which graph
  await section(page, '1')
    .getByRole('tree', { name: 'Namespaces' })
    .getByRole('link', { name: 'Tale' })
    .click()
  await expect(page).toHaveURL(/#\/g\/1\/page\/Tale$/)
  await expect(main(page).getByText('tale of beta')).toBeVisible()
  await expect(main(page).getByText('saga of the first')).toHaveCount(0)
  await expect(section(page, '1')).toHaveClass(/active/)
  await expect(section(page, '')).not.toHaveClass(/active/)
  await expect(main(page).getByTestId('graph-chip')).toContainText('Beta')
  await expect(page).toHaveTitle('Tale — Beta')
  // Recent belongs to the graph of the section
  await expect(section(page, '1').getByTestId('recent-pages')).toContainText('Tale')
  await expect(section(page, '').getByTestId('recent-pages')).not.toContainText('Tale')

  // an edit goes to the graph on the page area
  await main(page).getByText('tale of beta').click()
  await page.keyboard.press('End')
  await page.keyboard.type(' and more')
  await page.keyboard.press('Escape')
  await expect.poll(() => beta.read('pages/Tale.md')).toContain('tale of beta and more')
  expect(backend.exists('pages/Tale.md')).toBe(false)

  // and back: the first graph's pages, the first section marked
  await section(page, '').getByRole('link', { name: 'Journals' }).click()
  await expect(section(page, '')).toHaveClass(/active/)
  await expect(page).toHaveURL(/#\/journals$/)
  await section(page, '')
    .getByRole('tree', { name: 'Namespaces' })
    .getByRole('link', { name: 'Saga' })
    .click()
  await expect(main(page).getByText('saga of the first')).toBeVisible()
  await expect(main(page).getByTestId('graph-chip')).toContainText('graph')
})

test('what was typed is saved to its own graph when the other graph is opened at once', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Tale.md', '- tale of beta\n')
  backend.write('pages/Saga.md', '- saga of the first\n')
  await backend.open('/page/Saga')
  await main(page).getByText('saga of the first').click()
  await page.keyboard.press('End')
  await page.keyboard.type(' typed just now')
  // no Escape: the second graph is opened while the editor is still open
  await section(page, '1').getByTestId('section-fold').click()
  await section(page, '1')
    .getByRole('tree', { name: 'Namespaces' })
    .getByRole('link', { name: 'Tale' })
    .click()
  await expect(main(page).getByText('tale of beta')).toBeVisible()
  await expect.poll(() => backend.read('pages/Saga.md')).toContain('saga of the first typed just now')
  expect(beta.read('pages/Tale.md')).toBe('- tale of beta\n')
})

test('icons, favorites and padlocks of a section are the graph of that section', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Octo.md', '---\nicon: 🐙\n---\n- hi\n')
  backend.write('pages/Saga.md', '- plain\n')
  await beta.call('get_page', { name: 'Octo' })
  await beta.call('favorite_set', { page: 'Octo', favorite: true })
  await backend.open('/page/Saga')
  await section(page, '1').getByTestId('section-fold').click()
  await expect(section(page, '1').getByTestId('favorites')).toContainText('Octo')
  await expect(section(page, '').getByTestId('favorites')).toHaveCount(0)
  await expect(
    section(page, '1').getByRole('tree', { name: 'Namespaces' }).getByTestId('page-icon'),
  ).toHaveText('🐙')
  await expect(section(page, '').getByTestId('page-icon')).toHaveCount(0)
})

test('the second graph is closed from its section, and the page area goes back to the first', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Tale.md', '- tale of beta\n')
  await backend.open('/g/1/page/Tale')
  await expect(main(page).getByText('tale of beta')).toBeVisible()
  await expect(section(page, '1')).toHaveClass(/active/)
  await section(page, '1').getByTestId('section-close').click()
  await expect(section(page, '1')).toHaveCount(0)
  await expect(section(page, '')).toHaveClass(/active/)
  await expect(page).toHaveURL(/#\/journals$/)
  await expect(main(page).getByText('tale of beta')).toHaveCount(0)
  const slots = await backend.call<{ slot: string }[]>('slots')
  expect(slots.map((s) => s.slot)).toEqual([''])
  // a reload does not bring it back
  await page.reload()
  await expect(section(page, '1')).toHaveCount(0)
})

test('a second graph is opened from the list of this device, and again after a reload', async ({
  backend,
  page,
}) => {
  const other = `${backend.dir}-other`
  await backend.call('open_graph', { path: other, create: true })
  await backend.call('open_graph', { path: backend.dir, create: false })
  await backend.open('/journals')
  await expect(section(page, '1')).toHaveCount(0)
  await page.getByTestId('open-second').click()
  await page
    .getByTestId('second-menu')
    .getByRole('menuitem', { name: /-other/ })
    .click()
  await expect(section(page, '1')).toBeVisible()
  await expect(section(page, '1').getByTestId('graph-switcher')).toContainText('-other')
  // the same graph is not offered twice, and the first section cannot take the graph of the second
  await expect(page.getByTestId('open-second')).toHaveCount(0)
  await page.reload()
  await expect(section(page, '1')).toBeVisible()
})

// ---- dragging between the graphs ------------------------------------------------------------------------------------
const dialog = (page: Page) => page.getByTestId('transfer-dialog')

test('a page dragged from one section to the other opens the dialog with the target fixed', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  backend.write('pages/Dragged.md', '- drag me over\n')
  await backend.open('/journals')
  const row = section(page, '')
    .getByRole('tree', { name: 'Namespaces' })
    .getByRole('link', { name: 'Dragged' })
  await row.dragTo(section(page, '1'))
  const d = dialog(page)
  await expect(d).toBeVisible()
  await expect(d.getByTestId('transfer-source')).toHaveText('graph')
  await expect(d.getByTestId('transfer-target')).toHaveCount(1)
  await expect(d.locator('label.choice', { hasText: 'Beta' })).toBeVisible()
  await expect(d.getByTestId('transfer-plan')).toContainText('New page Dragged')
  await d.getByTestId('transfer-go').click()
  await expect.poll(() => beta.exists('pages/Dragged.md')).toBe(true)
  expect(backend.exists('pages/Dragged.md')).toBe(true)
})

test('a block dragged onto the other section is added to the page of the same name, and a move empties its page', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Notes.md', '- already in beta\n')
  backend.write('pages/Notes.md', '- stays\n- travels along\n  - with its child\n')
  await beta.call('get_page', { name: 'Notes' })
  await backend.open('/page/Notes')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).nth(1).dragTo(section(page, '1'))
  const d = dialog(page)
  await expect(d.getByTestId('transfer-page-name')).toHaveValue('Notes')
  await d.getByTestId('transfer-move').check()
  await expect(d.getByTestId('transfer-plan')).toContainText('Added to Notes')
  await d.getByTestId('transfer-go').click()
  await expect
    .poll(() => beta.read('pages/Notes.md'))
    .toBe('- already in beta\n- travels along\n  - with its child\n')
  await expect.poll(() => backend.read('pages/Notes.md')).toBe('- stays\n')
  await expect(blocks.getByText('travels along')).toHaveCount(0)
})

test('a page of the first graph dragged onto the page area of the second graph is taken there', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Tale.md', '- tale of beta\n')
  backend.write('pages/Saga.md', '- saga of the first\n')
  await backend.open('/g/1/page/Tale')
  await expect(main(page).getByText('tale of beta')).toBeVisible()
  await section(page, '')
    .getByRole('tree', { name: 'Namespaces' })
    .getByRole('link', { name: 'Saga' })
    .dragTo(main(page))
  const d = dialog(page)
  await expect(d.getByTestId('transfer-source')).toHaveText('graph')
  await d.getByTestId('transfer-go').click()
  await expect.poll(() => beta.exists('pages/Saga.md')).toBe(true)
  // nothing was dropped on its own graph: a page dragged within the first section does nothing
  await section(page, '1')
    .getByRole('tree', { name: 'Namespaces' })
    .getByRole('link', { name: 'Tale' })
    .dragTo(main(page))
  await expect(dialog(page)).toHaveCount(0)
})

test('any section folds to its header with the triangle at its left, and the choice is remembered', async ({
  backend,
  page,
}) => {
  backend.write('pages/Saga.md', '- saga text\n')
  await backend.open('/page/Saga')
  // one graph: the section can be folded as well
  const fold = section(page, '').getByTestId('section-fold')
  await expect(fold).toHaveAttribute('aria-expanded', 'true')
  await expect(section(page, '').getByRole('navigation')).toBeVisible()
  await fold.click()
  await expect(fold).toHaveAttribute('aria-expanded', 'false')
  await expect(section(page, '').getByRole('navigation')).toHaveCount(0)
  await expect(section(page, '').getByTestId('recent-pages')).toHaveCount(0)
  await expect(section(page, '').getByTestId('graph-switcher')).toBeVisible()
  // the page area is not touched by folding
  await expect(main(page).getByText('saga text')).toBeVisible()
  // the triangle points right when folded and down when open (computed transform), and is at the left of the name
  const box = await fold.boundingBox()
  const name = await section(page, '').getByTestId('graph-switcher').boundingBox()
  expect(box!.x + box!.width).toBeLessThanOrEqual(name!.x + 1)
  await page.reload()
  await expect(section(page, '').getByRole('navigation')).toHaveCount(0)
  await section(page, '').getByTestId('section-fold').click()
  await expect(section(page, '').getByRole('navigation')).toBeVisible()
})

test('with two graphs both can be folded, and the active one stays folded while it is used', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Tale.md', '- tale of beta\n')
  backend.write('pages/Saga.md', '- saga text\n')
  await backend.open('/page/Saga')
  await section(page, '1').getByTestId('section-fold').click()
  await expect(section(page, '1').getByRole('navigation')).toBeVisible()
  // fold the first one (the active graph): it stays folded, the page area keeps its page
  await section(page, '').getByTestId('section-fold').click()
  await expect(section(page, '').getByRole('navigation')).toHaveCount(0)
  await expect(main(page).getByText('saga text')).toBeVisible()
  // opening a page in the second graph makes it active; the first stays folded, the second open
  await section(page, '1')
    .getByRole('tree', { name: 'Namespaces' })
    .getByRole('link', { name: 'Tale' })
    .click()
  await expect(section(page, '1')).toHaveClass(/active/)
  await expect(section(page, '').getByRole('navigation')).toHaveCount(0)
  // fold the active one too
  await section(page, '1').getByTestId('section-fold').click()
  await expect(section(page, '1').getByRole('navigation')).toHaveCount(0)
  await expect(main(page).getByText('tale of beta')).toBeVisible()
  // the second graph keeps the ✕ and its name while folded
  await expect(section(page, '1').getByTestId('section-close')).toBeVisible()
  await expect(section(page, '1').getByTestId('graph-switcher')).toContainText('Beta')
})
