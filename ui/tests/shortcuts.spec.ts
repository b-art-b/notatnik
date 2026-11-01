import { expect, test } from './fixtures'

// The sidebar places have keys: Alt+1..5 anywhere, `g` then a letter when no text field has the focus; the key
// is shown next to the entry while it is hovered.
test('Alt+digit goes to Journals, Agenda, All pages and Settings, and Alt+4 opens search', async ({
  backend,
  page,
}) => {
  backend.write('pages/s.md', '- a block\n')
  await backend.open('/page/s')
  await page.keyboard.press('Alt+2')
  await expect(page).toHaveURL(/#\/agenda/)
  await page.keyboard.press('Alt+3')
  await expect(page).toHaveURL(/#\/pages/)
  await page.keyboard.press('Alt+5')
  await expect(page).toHaveURL(/#\/settings/)
  await page.keyboard.press('Alt+1')
  await expect(page).toHaveURL(/#\/journals/)
  await page.keyboard.press('Alt+4')
  await expect(page.getByRole('dialog', { name: /search/i })).toBeVisible()
})

test('Alt+digit works while a block is being edited', async ({ backend, page }) => {
  backend.write('pages/s2.md', '- typing here\n')
  await backend.open('/page/s2')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('typing here').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Alt+2')
  await expect(page).toHaveURL(/#\/agenda/)
})

test('g then a letter goes there, but not while typing', async ({ backend, page }) => {
  backend.write('pages/s3.md', '- one\n')
  await backend.open('/page/s3')
  await page.keyboard.press('g')
  await page.keyboard.press('a')
  await expect(page).toHaveURL(/#\/agenda/)
  await page.keyboard.press('g')
  await page.keyboard.press('p')
  await expect(page).toHaveURL(/#\/pages/)
  // in a text field `g` and `a` are just letters
  await backend.open('/page/s3')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('one', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('ga')
  await expect(page).toHaveURL(/#\/page\/s3/)
})

test('hovering a sidebar entry shows its key, and the tooltip names it', async ({ backend, page }) => {
  backend.write('pages/s4.md', '- one\n')
  await backend.open('/page/s4')
  const nav = page.locator('.left nav')
  const agenda = nav.getByRole('link', { name: 'Agenda' })
  await expect(agenda).toHaveAttribute('title', 'Agenda (Alt+2, or g then a)')
  const hint = agenda.locator('.keyhint')
  await expect(hint).toHaveText('Alt+2')
  // opacity 0 counts as visible for Playwright: assert the computed style
  await expect(hint).toHaveCSS('opacity', '0')
  await agenda.hover()
  await expect(hint).toHaveCSS('opacity', '1')
})
