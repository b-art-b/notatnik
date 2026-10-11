import { expect, test } from './fixtures'

test('Hierarchy lists every page below the page as a path of links', async ({ backend, page }) => {
  backend.write('pages/Z/Partner Beta.md', '- the partner\n')
  backend.write('pages/Z/Partner Beta/Renewal.md', '- renewal\n')
  backend.write('pages/Z/Partner Beta/Renewal/Client Two.md', '- client\n')
  backend.write('pages/Zebra.md', '- not below Z\n')
  await backend.open('/page/Z%2FPartner%20Beta')
  const h = page.getByTestId('hierarchy')
  await expect(h.getByRole('heading', { name: 'Hierarchy' })).toBeVisible()
  const rows = h.locator('li')
  await expect(rows).toHaveCount(2) // Renewal and Client Two; the page itself and `Zebra` are not listed
  // each row is the whole path, each segment a link to that page
  await expect(rows.nth(0)).toHaveText('Z/Partner Beta/Renewal')
  await expect(rows.nth(1)).toHaveText('Z/Partner Beta/Renewal/Client Two')
  await expect(rows.nth(1).locator('a')).toHaveCount(4)
  await expect(rows.nth(1).locator('a[data-page="Z/Partner Beta"]')).toBeVisible()
  await rows.nth(1).locator('a[data-page="Z/Partner Beta/Renewal"]').click()
  await expect(page).toHaveURL(/Z%2FPartner%20Beta%2FRenewal/)
  // from there, only the pages below it
  await expect(page.getByTestId('hierarchy').locator('li')).toHaveCount(1)
})

test('a page without subpages has no Hierarchy section', async ({ backend, page }) => {
  backend.write('pages/Solo.md', '- alone\n')
  await backend.open('/page/Solo')
  await expect(page.getByText('alone')).toBeVisible()
  await expect(page.getByTestId('hierarchy')).toHaveCount(0)
})
