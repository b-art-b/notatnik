import { expect, test } from './fixtures'

// Ctrl+Alt+N: a new page by name, its first block ready for typing; nothing is written until something is typed.
test('Ctrl+Alt+N asks for a name and opens the new page ready for writing', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+Alt+n')
  const dialog = page.getByRole('dialog', { name: 'New page' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByLabel('Page name')).toBeFocused()
  await dialog.getByLabel('Page name').fill('people/Nowa Osoba')
  await expect(dialog).toContainText('Enter creates “people/Nowa Osoba”')
  await page.keyboard.press('Enter')
  await expect(dialog).toHaveCount(0)
  await expect(page).toHaveURL(/#\/page\/people%2FNowa%20Osoba/)
  // the editor of its first block is open; leaving it empty creates no file
  await expect(page.getByTestId('block-editor')).toBeFocused()
  expect(backend.exists('pages/people/Nowa Osoba.md')).toBe(false)
  await page.keyboard.type('first note')
  await expect.poll(() => backend.exists('pages/people/Nowa Osoba.md')).toBe(true)
  await expect.poll(() => backend.read('pages/people/Nowa Osoba.md')).toBe('- first note\n')
})

test('an existing page is opened, not made again; Escape cancels; a bad name is explained', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  await page.getByTestId('new-page').click()
  const dialog = page.getByRole('dialog', { name: 'New page' })
  await dialog.getByLabel('Page name').fill('inbox')
  await expect(dialog).toContainText('“inbox” exists: Enter opens it')
  await expect(dialog.getByRole('listbox', { name: 'Existing pages' })).toContainText('inbox')
  await page.keyboard.press('Enter')
  await expect(page).toHaveURL(/#\/page\/inbox/)
  await expect(page.getByRole('tree', { name: 'Blocks' })).toBeVisible()
  // Escape closes it and nothing happens
  await page.keyboard.press('Control+Alt+n')
  await expect(dialog).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(dialog).toHaveCount(0)
  await expect(page).toHaveURL(/#\/page\/inbox/)
  // a name the server refuses stays in the dialog with the reason
  await page.keyboard.press('Control+Alt+n')
  await dialog.getByLabel('Page name').fill('../x')
  await page.keyboard.press('Enter')
  await expect(dialog.getByRole('alert')).toContainText(/relative|invalid/i)
})

test('the command palette offers to create a page that does not exist', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('zzz brand new')
  await page.getByRole('button', { name: /Create page “zzz brand new”/ }).click()
  await expect(page).toHaveURL(/#\/page\/zzz%20brand%20new/)
  await expect(page.getByTestId('block-editor')).toBeFocused()
})
