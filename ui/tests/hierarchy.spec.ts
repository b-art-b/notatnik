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

test('a page without subpages still offers "Add subpage"; the field starts with the page path', async ({
  backend,
  page,
}) => {
  backend.write('pages/Solo.md', '- alone\n')
  await backend.open('/page/Solo')
  await expect(page.getByText('alone')).toBeVisible()
  const h = page.getByTestId('hierarchy')
  await expect(h.locator('li')).toHaveCount(0)
  await h.getByRole('button', { name: '+ Add subpage' }).click()
  const field = h.getByLabel('Name of the new subpage')
  await expect(field).toBeFocused()
  await expect(field).toHaveValue('Solo/')
  await page.keyboard.type('Notes')
  await page.keyboard.press('Enter')
  await expect(page).toHaveURL(/Solo%2FNotes/)
  // the new page opens ready for typing; its parent now lists it
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('first')
  await expect.poll(() => backend.exists('pages/Solo/Notes.md')).toBe(true)
  await backend.open('/page/Solo')
  await expect(page.getByTestId('hierarchy').locator('li')).toHaveText('Solo/Notes')
})

test('the palette command opens the field; Escape closes it; the bare prefix is refused', async ({
  backend,
  page,
}) => {
  backend.write('pages/Solo.md', '- alone\n')
  await backend.open('/page/Solo')
  await page.keyboard.press('Control+k')
  await page.getByRole('dialog').getByRole('textbox').fill('New subpage')
  await page.keyboard.press('Enter')
  const field = page.getByTestId('hierarchy').getByLabel('Name of the new subpage')
  await expect(field).toBeFocused()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('alert').filter({ hasText: 'Type a name' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(field).toHaveCount(0)
})
