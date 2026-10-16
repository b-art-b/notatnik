import { expect, test } from './fixtures'

// Ctrl+K is the way to a new page: type a name that matches no page and create it; its first block opens for
// typing, and nothing is written until something is typed.
test('Ctrl+K, a new name, Ctrl+Enter: the page opens ready for writing', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  const palette = page.getByRole('dialog', { name: 'Command palette' })
  await expect(palette).toBeVisible()
  const field = palette.getByLabel('Command or page')
  await expect(field).toBeFocused()
  await field.fill('people/Nowa Osoba')
  await expect(palette.getByRole('button', { name: /Create page “people\/Nowa Osoba”/ })).toBeVisible()
  await page.keyboard.press('Control+Enter')
  await expect(palette).toHaveCount(0)
  await expect(page).toHaveURL(/#\/page\/people%2FNowa%20Osoba/)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  expect(backend.exists('pages/people/Nowa Osoba.md')).toBe(false)
  await page.keyboard.type('first note')
  await expect.poll(() => backend.exists('pages/people/Nowa Osoba.md')).toBe(true)
  await expect.poll(() => backend.read('pages/people/Nowa Osoba.md')).toBe('- first note\n')
})

test('Enter on the create row works too, and an existing name offers no create row', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  const palette = page.getByRole('dialog', { name: 'Command palette' })
  await palette.getByLabel('Command or page').fill('zzz brand new')
  await expect(palette.getByRole('option').first()).toContainText('Create page “zzz brand new”')
  await page.keyboard.press('Enter')
  await expect(page).toHaveURL(/#\/page\/zzz%20brand%20new/)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  // an existing page: no create row, Enter opens it
  await page.keyboard.press('Escape')
  await page.keyboard.press('Control+k')
  await palette.getByLabel('Command or page').fill('inbox')
  await expect(palette.getByRole('option').first()).toContainText('inbox')
  await expect(palette.getByRole('button', { name: /Create page/ })).toHaveCount(0)
  await page.keyboard.press('Enter')
  await expect(page).toHaveURL(/#\/page\/inbox/)
})

test('a name the app cannot use is explained and the palette stays open', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  const palette = page.getByRole('dialog', { name: 'Command palette' })
  await palette.getByLabel('Command or page').fill('../x')
  await page.keyboard.press('Control+Enter')
  await expect(palette.getByRole('alert')).toContainText(/relative|invalid/i)
  await expect(palette).toBeVisible()
  // Ctrl+K toggles it closed again
  await page.keyboard.press('Control+k')
  await expect(palette).toHaveCount(0)
})
