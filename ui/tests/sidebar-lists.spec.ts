import { expect, test } from './fixtures'

// The pages in the sidebar look the same in the namespace tree, in the groups, in Recent: the text color, the page you
// are on in bold with a background, names that wrap continue under the text and not under the icon.
const sidebar = (page: import('@playwright/test').Page) =>
  page.getByRole('complementary', { name: 'Sidebar' })

test('the tree, the groups and Recent show a page the same way', async ({ backend, page }) => {
  backend.write('pages/Notes.md', '---\ntype: note\nicon: 📅\n---\n- a\n')
  backend.write('pages/Notes/Plain.md', '---\ntype: note\n---\n- a\n')
  backend.write(
    'pages/Notes/A very long name that has to wrap in a narrow sidebar of the app.md',
    '---\ntype: note\n---\n- a\n',
  )
  await backend.open('/page/Notes')
  await sidebar(page).getByRole('button', { name: 'Expand all' }).click()
  const text = await page.evaluate(() => getComputedStyle(document.body).color)

  const inTree = sidebar(page).getByRole('tree', { name: 'Namespaces' }).locator('a[data-ns="Notes"]')
  const inRecent = sidebar(page).getByTestId('recent-pages').locator('a[data-recent="Notes"]')
  for (const a of [inTree, inRecent]) {
    // the page on screen: bold, on a background, in the text color (not the link color)
    await expect(a).toHaveCSS('font-weight', '600')
    await expect(a).toHaveCSS('color', text)
    await expect(a).not.toHaveCSS('background-color', 'rgba(0, 0, 0, 0)')
  }

  await sidebar(page).getByLabel('Group pages by').selectOption('type')
  await sidebar(page).getByRole('button', { name: 'Expand all' }).click()
  const groups = sidebar(page).getByTestId('grouped-pages')
  const other = groups.locator('a[data-page="Notes/Plain"]')
  await expect(other).toHaveCSS('color', text)
  await expect(other).toHaveCSS('font-weight', '400')
  await expect(other).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)')
  // the folder part of a name is quiet, the page's own name is in the text color
  await expect(other.locator('.ns')).toHaveText('Notes/')
  expect(await other.locator('.ns').evaluate((e) => getComputedStyle(e).color)).not.toBe(text)

  // a wrapped name continues under its text: the second line starts where the first line's text starts
  const long = groups.locator('a[data-page^="Notes/A very long"]')
  const label = long.locator('.label')
  const box = await label.boundingBox()
  const link = await long.boundingBox()
  const one = await other.locator('.label').boundingBox()
  expect(box!.height).toBeGreaterThan(one!.height * 1.4) // two lines
  expect(box!.x).toBeGreaterThan(link!.x + 10) // the text starts after the icon box, and so do the next lines
  // all names in the group start at the same place, with an icon or without
  const starts = await groups
    .locator('a.page-link .label')
    .evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().x)))
  expect(new Set(starts).size).toBe(1)
})
