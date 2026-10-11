import { expect, test } from './fixtures'

test('all pages: list with filter, sorting and toggles; namespaces mode; sidebar link', async ({
  backend,
  page,
}) => {
  backend.write('pages/zeta.md', '- one\n- two\n- three\n')
  backend.write('pages/alpha.md', '- only block with [[zeta]]\n')
  await backend.open('/')
  await page.getByRole('link', { name: 'All pages' }).click()
  await expect(page).toHaveURL(/#\/pages/)
  const table = page.getByTestId('pages-table')
  await expect(table).toBeVisible()
  const total = Number(await page.getByTestId('pages-count').innerText())
  expect(total).toBeGreaterThan(5)

  // journals are hidden by default and can be shown
  await expect(table.locator('a[data-page^="2026-"]')).toHaveCount(0)
  await page.getByLabel('Journals').check()
  await expect(table.locator('a[data-page^="2026-"]').first()).toBeVisible()
  await page.getByLabel('Journals').uncheck()

  // filter
  await page.getByLabel('Filter pages').fill('zet')
  await expect(page.getByTestId('pages-count')).toHaveText('1')
  await expect(table.locator('a[data-page="zeta"]')).toBeVisible()
  const row = table.locator('tr', { has: page.locator('a[data-page="zeta"]') })
  await expect(row.locator('td').nth(1)).toHaveText('3') // blocks
  await expect(row.locator('td').nth(2)).toHaveText('1') // linked from alpha
  await page.getByLabel('Filter pages').fill('')

  // sort by blocks, biggest first: zeta (3 blocks) comes before alpha (1)
  await page.getByRole('button', { name: /^Blocks/ }).click()
  const names = await table.locator('tbody a[data-page]').evaluateAll((as) => as.map((a) => a.textContent))
  expect(names.indexOf('zeta')).toBeLessThan(names.indexOf('alpha'))

  // pages that exist only as link targets are marked
  await page.getByLabel('Pages without a file').uncheck()
  await expect(table.getByText('no file')).toHaveCount(0)
  await page.getByLabel('Pages without a file').check()
  await expect(table.getByText('no file').first()).toBeVisible()

  // namespaces mode shows the tree; a page link opens the page
  await page.getByRole('button', { name: 'Namespaces' }).click()
  await expect(page.getByTestId('pages-tree')).toBeVisible()
  await page.getByTestId('pages-tree').locator('a[data-ns="zeta"]').click()
  await expect(page).toHaveURL(/#\/page\/zeta/)
})
