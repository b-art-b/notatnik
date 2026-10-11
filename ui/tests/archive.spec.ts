import { expect, test } from './fixtures'

test('archiving hides a page and what is below it, and unarchiving brings it back', async ({
  backend,
  page,
}) => {
  backend.write('pages/Team.md', '- team home\n')
  backend.write('pages/Team/anna.md', '- TODO call anna\n')
  backend.write('pages/Other.md', '- other page\n')
  await backend.open('/page/Team')
  const sidebar = page.getByRole('complementary', { name: 'Sidebar' })
  await expect(sidebar.locator('[data-ns="Team"]')).toBeVisible()
  await page.getByTestId('archive-toggle').click()
  await expect.poll(() => backend.read('pages/Team.md')).toMatch(/^---\narchived: true\n---\n/)
  // gone from the tree (the page itself and the one below it), the page shows a banner
  await expect(sidebar.getByTestId('archived-banner')).toHaveCount(0)
  await expect(page.getByTestId('archived-banner')).toContainText('Archived')
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
  await page.getByTestId('show-archived').check()
  await expect(page.getByTestId('pages-count')).toHaveText('2')
  await expect(page.getByTestId('pages-table')).toContainText('Team/anna')
  // unarchive from the page
  await page.evaluate(() => (location.hash = '#/page/Team'))
  await page.getByTestId('archive-toggle').click()
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
