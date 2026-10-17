import { expect, test } from './fixtures'

// The history of graphs: shown on the start screen (the last one selected), removable from the list.
test('the start screen lists the graphs, selects the last one, and a graph can be removed from the list', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  const list = page.getByTestId('graph-history')
  await expect(list).toBeVisible()
  const row = list.getByRole('listitem').first()
  await expect(row).toContainText('graph')
  await expect(row).toContainText('last opened')
  await expect(row).toContainText('open now')
  await expect(row).toContainText(backend.dir)
  // Enter opens the selected one: the start screen focuses the Open button of the last graph
  await expect(row.getByRole('button', { name: 'Open' })).toBeFocused()
  // another graph in the history can be removed from the list, the open one too (it is only the list)
  await page.getByLabel('Graph folder').fill(`${backend.dir}-second`)
  await page.getByRole('button', { name: 'Open / create' }).click()
  await expect(page.getByTestId('journals')).toBeVisible({ timeout: 10000 })
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  await expect(list.getByRole('listitem')).toHaveCount(2)
  await expect(list.getByRole('listitem').first()).toContainText('-second')
  await expect(list.getByRole('listitem').first()).toContainText('last opened')
  await list
    .getByRole('button', { name: /^Remove graph.* from the list/ })
    .last()
    .click()
  await expect(list.getByRole('listitem')).toHaveCount(1)
  // back to the graph that is open
  await page.getByRole('button', { name: /Back to/ }).click()
  await expect(page.getByTestId('journals')).toBeVisible()
})

// The folder chooser is the system's own dialog (desktop app only); the browser build keeps the path field alone
test('the browser build has no folder chooser button, only the path field', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  await expect(page.getByLabel('Graph folder')).toBeVisible()
  await expect(page.getByTestId('browse-folder')).toHaveCount(0)
})

// The graph name at the top of the sidebar opens a menu: the other graphs to switch to, the start screen.
test('the sidebar menu switches to another graph and leads to the start screen', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  const switcher = page.getByTestId('graph-switcher')
  await expect(switcher).toContainText('graph')
  // no other graph yet: only the two entries that lead to the start screen
  await switcher.click()
  const menu = page.getByRole('menu', { name: 'Graphs' })
  await expect(menu.getByRole('menuitem')).toHaveText(['Add new graph…', 'All graphs…'])
  await page.keyboard.press('Escape')
  await expect(menu).toBeHidden()
  // a second graph is opened from the start screen, then the menu offers the first one
  await switcher.click()
  await menu.getByRole('menuitem', { name: 'Add new graph…' }).click()
  await page.getByLabel('Graph folder').fill(`${backend.dir}-second`)
  await page.getByRole('button', { name: 'Open / create' }).click()
  await expect(switcher).toContainText('-second', { timeout: 10000 })
  await switcher.click()
  await expect(menu).toContainText('Switch to:')
  await menu.getByRole('menuitem', { name: 'graph', exact: true }).click()
  await expect(switcher).not.toContainText('-second', { timeout: 10000 })
  // a click elsewhere closes the menu
  await switcher.click()
  await page.getByTestId('journals').click({ position: { x: 5, y: 5 } })
  await expect(menu).toBeHidden()
})
