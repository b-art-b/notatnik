import { expect, test } from './fixtures'

// Favorite pages (NS-26): the star of a page, the sidebar list above Recent, its order, and what a rename and a delete do.
const sidebar = (page: import('@playwright/test').Page) =>
  page.getByRole('complementary', { name: 'Sidebar' })
/** The names in `favorites = [...]` of the settings file, as written there. */
const favoritesInConfig = (backend: { read: (rel: string) => string }) => {
  const m = /^favorites = \[([^\]]*)\]/m.exec(backend.read('.notatnik/config.toml'))
  return m ? [...m[1].matchAll(/"([^"]*)"/g)].map((x) => x[1]) : []
}
const entries = (page: import('@playwright/test').Page) =>
  sidebar(page).getByTestId('favorites').locator('a[data-favorite]')

test('a page is starred, shows in the sidebar and in the settings file, and is un-starred again', async ({
  backend,
  page,
}) => {
  backend.write('pages/Ferret.md', '- a\n')
  backend.write('pages/Heron.md', '- b\n')
  await backend.open('/page/Ferret')
  // no favorites yet: no section at all
  await expect(sidebar(page).getByTestId('favorites')).toHaveCount(0)
  const star = page.getByRole('main').getByTestId('favorite-toggle')
  await expect(star).toHaveAttribute('aria-pressed', 'false')
  await star.click()
  await expect(star).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('main').getByTestId('title-star')).toBeVisible()
  await expect(entries(page)).toHaveText(['Ferret'])
  // it is in the graph's settings, so it syncs; a reload keeps it
  await expect.poll(() => favoritesInConfig(backend)).toEqual(['Ferret'])
  await page.reload()
  await expect(entries(page)).toHaveText(['Ferret'])

  // a second one, from the palette
  await backend.open('/page/Heron')
  await page.keyboard.press('Control+k')
  await page.getByRole('dialog').getByRole('textbox').fill('Add or remove favorite')
  await page.keyboard.press('Enter')
  await expect(entries(page)).toHaveText(['Ferret', 'Heron'])

  // removed with the × of its row, and with the star
  await sidebar(page).getByRole('button', { name: 'Remove Ferret from the favorites' }).click()
  await expect(entries(page)).toHaveText(['Heron'])
  await page.getByRole('main').getByTestId('favorite-toggle').click()
  await expect(sidebar(page).getByTestId('favorites')).toHaveCount(0)
  await expect(page.getByRole('main').getByTestId('title-star')).toHaveCount(0)
})

test('the order is changed with Alt+arrows and by dragging, and a rename and a delete are followed', async ({
  backend,
  page,
}) => {
  for (const n of ['Ferret', 'Heron', 'Ibis']) backend.write(`pages/${n}.md`, '- x\n')
  backend.write('pages/aviary/Wren.md', '- x\n')
  await backend.open('/')
  for (const n of ['Ferret', 'Heron', 'Ibis', 'aviary/Wren'])
    await backend.call('favorite_set', { page: n, favorite: true })
  await page.reload()
  await expect(entries(page)).toHaveText(['Ferret', 'Heron', 'Ibis', 'aviary/Wren'])

  // Alt+↓ moves the focused row down and keeps the focus on it
  await entries(page).first().focus()
  await page.keyboard.press('Alt+ArrowDown')
  await expect(entries(page)).toHaveText(['Heron', 'Ferret', 'Ibis', 'aviary/Wren'])
  await expect(sidebar(page).locator('a[data-favorite="Ferret"]')).toBeFocused()
  await page.keyboard.press('Alt+ArrowUp')
  await expect(entries(page)).toHaveText(['Ferret', 'Heron', 'Ibis', 'aviary/Wren'])

  // dragging a row onto another puts it there
  await entries(page).nth(2).dragTo(entries(page).nth(0))
  await expect(entries(page)).toHaveText(['Ibis', 'Ferret', 'Heron', 'aviary/Wren'])
  await expect.poll(() => favoritesInConfig(backend)).toEqual(['Ibis', 'Ferret', 'Heron', 'aviary/Wren'])

  // a rename (and a namespace with the favorites below it) is followed; a delete drops the page
  await backend.call('rename_apply', { old: 'Ferret', new: 'Osprey' })
  await backend.call('rename_apply', { old: 'aviary', new: 'flock' })
  await expect(entries(page)).toHaveText(['Ibis', 'Osprey', 'Heron', 'flock/Wren'])
  await backend.call('delete_pages', { page: 'Heron', include_subpages: false })
  await expect(entries(page)).toHaveText(['Ibis', 'Osprey', 'flock/Wren'])
})

test('at most twenty favorites: the 21st says so', async ({ backend, page }) => {
  for (let i = 0; i < 21; i++) backend.write(`pages/P${String(i).padStart(2, '0')}.md`, '- x\n')
  await backend.open('/')
  for (let i = 0; i < 20; i++)
    await backend.call('favorite_set', { page: `P${String(i).padStart(2, '0')}`, favorite: true })
  await page.reload()
  await backend.open('/page/P20')
  await page.getByRole('main').getByTestId('favorite-toggle').click()
  await expect(page.getByText(/at most 20 favorites/)).toBeVisible()
  await expect(page.getByRole('main').getByTestId('favorite-toggle')).toHaveAttribute('aria-pressed', 'false')
  await expect(entries(page)).toHaveCount(20)
})
