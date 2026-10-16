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
