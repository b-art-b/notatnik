import { expect, test } from './fixtures'

test('hiding a page hides what is below it, and unhiding brings it back', async ({ backend, page }) => {
  backend.write('pages/Team.md', '- team home\n')
  backend.write('pages/Team/anna.md', '- TODO call anna\n')
  backend.write('pages/Other.md', '- other page\n')
  await backend.open('/page/Team')
  const sidebar = page.getByRole('complementary', { name: 'Sidebar' })
  await expect(sidebar.locator('[data-ns="Team"]')).toBeVisible()
  await page.getByTestId('hide-toggle').click()
  await expect.poll(() => backend.read('pages/Team.md')).toMatch(/^---\narchived: true\n---\n/)
  // gone from the tree (the page itself and the one below it), the page shows a banner
  await expect(sidebar.getByTestId('hidden-banner')).toHaveCount(0)
  await expect(page.getByTestId('hidden-banner')).toContainText('Hidden')
  await expect(sidebar.locator('[data-ns="Team"]')).toHaveCount(0)
  await expect(sidebar.locator('[data-ns="Other"]')).toBeVisible()
  // its tasks are not on the agenda
  await page.evaluate(() => (location.hash = '#/agenda'))
  await expect(page.getByRole('heading', { name: 'Agenda' })).toBeVisible()
  await expect(page.locator('main')).not.toContainText('call anna')
  // All pages lists it only on request
  await page.evaluate(() => (location.hash = '#/pages'))
  await page.getByLabel('Filter pages').fill('Team')
  await expect(page.getByTestId('pages-count')).toHaveText('0')
  await page.getByTestId('hidden-filter').selectOption('with')
  await expect(page.getByTestId('pages-count')).toHaveText('2')
  await expect(page.getByTestId('pages-table')).toContainText('Team/anna')
  // unhide from the page
  await page.evaluate(() => (location.hash = '#/page/Team'))
  await page.getByTestId('hide-toggle').click()
  await expect.poll(() => backend.read('pages/Team.md')).not.toContain('archived')
  await expect(sidebar.locator('[data-ns="Team"]')).toBeVisible()
})

test('deleting a page moves it to the trash, keeps links, and Undo brings it back', async ({
  backend,
  page,
}) => {
  backend.write('pages/Old.md', '- old content\n')
  backend.write('pages/Keeper.md', '- see [[Old]]\n')
  await backend.open('/page/Old')
  await page.getByTestId('delete-page').click()
  const dialog = page.getByRole('dialog', { name: 'Delete page' })
  await expect(dialog).toContainText('1 page links to it')
  await dialog.getByRole('button', { name: 'Delete' }).click()
  await expect.poll(() => backend.exists('pages/Old.md')).toBe(false)
  expect(backend.read('pages/Keeper.md')).toBe('- see [[Old]]\n') // the link stays
  const toast = page.getByRole('status').filter({ hasText: 'Deleted' })
  await expect(toast).toBeVisible()
  await toast.getByRole('button', { name: 'Undo' }).click()
  await expect.poll(() => backend.exists('pages/Old.md')).toBe(true)
  expect(backend.read('pages/Old.md')).toBe('- old content\n')
  await expect(page).toHaveURL(/#\/page\/Old/)
})

test('deleting a namespace can take its subpages along', async ({ backend, page }) => {
  backend.write('pages/Proj.md', '- home\n')
  backend.write('pages/Proj/a.md', '- a\n')
  backend.write('pages/Proj/b.md', '- b\n')
  await backend.open('/page/Proj')
  await page.getByTestId('delete-page').click()
  const dialog = page.getByRole('dialog', { name: 'Delete page' })
  await dialog.getByLabel(/Also delete the 2 pages/).check()
  await dialog.getByRole('button', { name: 'Delete' }).click()
  for (const f of ['Proj.md', 'Proj/a.md', 'Proj/b.md']) {
    await expect.poll(() => backend.exists(`pages/${f}`)).toBe(false)
  }
})

test('All pages: a row hides, unhides and deletes a page without leaving the list', async ({
  backend,
  page,
}) => {
  backend.write('pages/Alpha.md', '- alpha\n')
  backend.write('pages/Beta.md', '- beta\n')
  await backend.open('/pages')
  const table = page.getByTestId('pages-table')
  const row = (name: string) => table.locator('tr', { has: page.locator(`a[data-page="${name}"]`) })

  // hide: the row disappears (hidden pages are not listed), the file gets the property, Undo brings it back
  await row('Alpha').getByTestId('row-hide').click()
  await expect.poll(() => backend.read('pages/Alpha.md')).toMatch(/^---\narchived: true\n---\n/)
  await expect(row('Alpha')).toHaveCount(0)
  await page.getByRole('button', { name: 'Undo' }).click()
  await expect(row('Alpha')).toBeVisible()
  expect(backend.read('pages/Alpha.md')).not.toContain('archived')

  // unhide from the list when hidden pages are shown
  await row('Alpha').getByTestId('row-hide').click()
  await page.getByTestId('hidden-filter').selectOption('with')
  await expect(row('Alpha').getByTestId('row-hide')).toHaveText('Unhide')
  await row('Alpha').getByTestId('row-hide').click()
  await expect.poll(() => backend.read('pages/Alpha.md')).not.toContain('archived')

  // delete: asks first, stays on the list, the file goes to the trash, Undo restores it
  await row('Beta').getByTestId('row-delete').click()
  await expect(page.getByRole('dialog', { name: 'Delete page' })).toContainText('Beta')
  await expect(page).toHaveURL(/#\/pages/)
  await page.getByRole('dialog', { name: 'Delete page' }).getByRole('button', { name: 'Delete' }).click()
  await expect(page).toHaveURL(/#\/pages/)
  await expect(row('Beta')).toHaveCount(0)
  expect(backend.exists('pages/Beta.md')).toBe(false)
  // earlier toasts may still be up: take the one about this deletion
  await page.locator('.toast', { hasText: 'Deleted' }).getByRole('button', { name: 'Undo' }).click()
  await expect(row('Beta')).toBeVisible()
  expect(backend.exists('pages/Beta.md')).toBe(true)
})

test('All pages, Namespaces view: a page hides and deletes from its line', async ({ backend, page }) => {
  backend.write('pages/Alpha.md', '- alpha\n')
  backend.write('pages/Beta.md', '- beta\n')
  await backend.open('/pages')
  await page.getByRole('button', { name: 'Namespaces' }).click()
  const tree = page.getByTestId('pages-tree')
  const line = (name: string) => tree.locator('.line', { has: page.locator(`a[data-ns="${name}"]`) })

  await line('Alpha').getByTestId('row-hide').click()
  await expect.poll(() => backend.read('pages/Alpha.md')).toMatch(/^---\narchived: true\n---\n/)
  await expect(line('Alpha')).toHaveCount(0) // hidden pages are not in the tree

  await line('Beta').getByTestId('row-delete').click()
  await page.getByRole('dialog', { name: 'Delete page' }).getByRole('button', { name: 'Delete' }).click()
  await expect(line('Beta')).toHaveCount(0)
  expect(backend.exists('pages/Beta.md')).toBe(false)
  await expect(page).toHaveURL(/#\/pages/)

  // the sidebar tree has no buttons
  await expect(page.getByRole('complementary', { name: 'Sidebar' }).getByTestId('row-hide')).toHaveCount(0)
})

test('a folder that has no page of its own is hidden from its page and from the list, and unhidden', async ({
  backend,
  page,
}) => {
  backend.write('pages/Docs/a.md', '- TODO a task in docs\n')
  backend.write('pages/Docs/b.md', '- b\n')
  backend.write('pages/Elsewhere.md', '- e\n')
  await backend.open('/page/Docs')
  const sidebar = page.getByRole('complementary', { name: 'Sidebar' })
  await expect(sidebar.locator('[data-ns="Docs"]')).toBeVisible()
  // the folder has no file: its page is empty, and it can be hidden all the same (nothing is created)
  await page.getByTestId('hide-toggle').click()
  await expect(sidebar.locator('[data-ns="Docs"]')).toHaveCount(0)
  await expect(page.getByTestId('hidden-banner')).toContainText('Hidden')
  expect(backend.exists('pages/Docs.md')).toBe(false)
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('hidden = ["Docs"]')
  // a page below it says why it is hidden, and has no Unhide of its own
  await page.evaluate(() => (location.hash = '#/page/Docs/a'))
  await expect(page.getByTestId('hidden-banner')).toContainText('because “Docs” is')
  await expect(page.getByTestId('hide-toggle')).toHaveText('Hide')
  // unhide from the folder's page
  await page.evaluate(() => (location.hash = '#/page/Docs'))
  await page.getByTestId('hide-toggle').click()
  await expect(sidebar.locator('[data-ns="Docs"]')).toBeVisible()
  await expect.poll(() => backend.read('.notatnik/config.toml')).not.toContain('hidden = ["Docs"]')
})

test('All pages: the hidden filter has three states, in the list and in the tree', async ({
  backend,
  page,
}) => {
  backend.write('pages/Team.md', '---\narchived: true\n---\n- t\n')
  backend.write('pages/Team/anna.md', '- a\n')
  backend.write('pages/Open.md', '- o\n')
  await backend.open('/pages')
  const names = () => page.getByTestId('pages-table').locator('a[data-page]')
  const filter = page.getByTestId('hidden-filter')
  await expect(filter).toHaveValue('without')
  await expect(names()).toContainText(['Open'])
  await expect(page.getByTestId('pages-table')).not.toContainText('Team')
  await filter.selectOption('with')
  await expect(page.getByTestId('pages-table')).toContainText('Team/anna')
  await expect(page.getByTestId('pages-table')).toContainText('Open')
  // hidden rows say so: the page itself and the one that is hidden with it
  const row = (n: string) =>
    page.getByTestId('pages-table').locator('tr', { has: page.locator(`a[data-page="${n}"]`) })
  await expect(row('Team').getByTestId('row-hidden-badge')).toHaveText('hidden')
  await expect(row('Team/anna').getByTestId('row-hidden-badge')).toHaveText('hidden with its folder')
  // only the hidden ones
  await filter.selectOption('only')
  await expect(page.getByTestId('pages-table')).toContainText('Team')
  await expect(page.getByTestId('pages-table')).not.toContainText('Open')
  await expect(page.getByTestId('pages-count')).toHaveText('2')
  // the tree: hidden pages are not there by default, shown with the filter, and "only" keeps the names above them
  await page.getByRole('button', { name: 'Namespaces' }).click()
  const tree = page.getByTestId('pages-tree')
  await expect(tree.locator('[data-ns="Team"]')).toBeVisible()
  await expect(tree.locator('[data-ns="Open"]')).toHaveCount(0)
  await filter.selectOption('without')
  await expect(tree.locator('[data-ns="Team"]')).toHaveCount(0)
  await expect(tree.locator('[data-ns="Open"]')).toBeVisible()
})

test('All pages: several rows are ticked and hidden or unhidden at once, with Undo', async ({
  backend,
  page,
}) => {
  for (const n of ['Alpha', 'Beta', 'Gamma']) backend.write(`pages/${n}.md`, `- ${n}\n`)
  backend.write('pages/Docs/a.md', '- a\n') // "Docs" has no file
  await backend.open('/pages')
  const table = page.getByTestId('pages-table')
  const tick = (n: string) =>
    table
      .locator('tr', { has: page.locator(`a[data-page="${n}"]`) })
      .getByTestId('row-select')
      .check()
  await tick('Alpha')
  await tick('Beta')
  await tick('Docs')
  const bar = page.getByTestId('selection-bar')
  await expect(bar).toContainText('3 selected')
  await page.getByTestId('selection-hide').click()
  await expect(bar).toHaveCount(0)
  await expect.poll(() => backend.read('pages/Alpha.md')).toMatch(/^---\narchived: true\n---\n/)
  await expect.poll(() => backend.read('pages/Beta.md')).toMatch(/^---\narchived: true\n---\n/)
  expect(backend.read('pages/Gamma.md')).toBe('- Gamma\n')
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('hidden = ["Docs"]')
  await expect(table).not.toContainText('Alpha')
  await expect(table).toContainText('Gamma')
  // Undo puts all three back
  await page.locator('.toast', { hasText: 'Hid 3 pages' }).getByRole('button', { name: 'Undo' }).click()
  await expect.poll(() => backend.read('pages/Alpha.md')).toBe('- Alpha\n')
  await expect(table).toContainText('Alpha')
  await expect.poll(() => backend.read('.notatnik/config.toml')).not.toContain('hidden = ["Docs"]')

  // all shown rows with the header tick, then unhide what is hidden (here: hide all, show them, unhide all)
  await table.getByTestId('select-all').check()
  await expect(bar).toContainText('selected')
  await page.getByTestId('selection-hide').click()
  await page.getByTestId('hidden-filter').selectOption('only')
  await expect(page.getByTestId('pages-count')).not.toHaveText('0')
  await table.getByTestId('select-all').check()
  await page.getByTestId('selection-unhide').click()
  await expect.poll(() => backend.read('pages/Gamma.md')).toBe('- Gamma\n')
  await expect(page.getByTestId('pages-count')).toHaveText('0')
})
