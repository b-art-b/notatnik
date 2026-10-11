import { expect, test } from './fixtures'

test('the sidebar lists recently opened pages, newest first', async ({ backend, page }) => {
  backend.write('pages/one.md', '- first\n')
  backend.write('pages/two.md', '- second\n')
  backend.write('pages/three.md', '- third\n')
  await backend.open('/journals')
  await expect(page.getByTestId('recent-pages')).toHaveCount(0) // nothing opened yet
  for (const n of ['one', 'two', 'three', 'one']) {
    await page.goto(page.url().replace(/#.*$/, '') + `#/page/${n}`)
    await expect(page.getByRole('heading', { level: 1 })).toHaveText(n)
  }
  const list = page.getByTestId('recent-pages')
  const names = () => list.locator('a').evaluateAll((as) => as.map((a) => a.getAttribute('data-recent')))
  // newest first, `one` moved up instead of appearing twice
  await expect.poll(names).toEqual(['one', 'three', 'two'])
  await expect(list.locator('a[data-recent="one"]')).toHaveAttribute('aria-current', 'page')
  // the list is remembered across a reload
  await page.reload()
  await expect.poll(names).toEqual(['one', 'three', 'two'])
  // clicking an entry opens the page
  await list.locator('a[data-recent="two"]').click()
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('two')
  await expect.poll(names).toEqual(['two', 'one', 'three'])
  await list.getByRole('button', { name: 'clear' }).click()
  await expect(page.getByTestId('recent-pages')).toHaveCount(0)
})

test('renaming a page keeps its place in the recent list', async ({ backend, page }) => {
  backend.write('pages/old name.md', '- body\n')
  backend.write('pages/other.md', '- other\n')
  await backend.open('/page/other')
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/old%20name')
  await page.getByTitle('Rename page').click()
  await page.getByLabel('New name').fill('new name')
  await page.getByRole('button', { name: 'Preview' }).click()
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  await expect(page).toHaveURL(/new%20name/)
  const names = page.getByTestId('recent-pages').locator('a')
  await expect(names.first()).toHaveAttribute('data-recent', 'new name')
  await expect(page.getByTestId('recent-pages').locator('a[data-recent="old name"]')).toHaveCount(0)
})
