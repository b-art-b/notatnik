import { expect, test } from './fixtures'

// Saved queries (the sidebar's list): the query's own page has an Edit form that keeps the query where it stands.
const saved = (page: import('@playwright/test').Page) => page.getByRole('complementary', { name: 'Sidebar' })

test('a saved query is edited and renamed from its page, in place', async ({ backend, page }) => {
  for (const [name, query] of [
    ['First', 'task: todo'],
    ['Second', 'task: done'],
    ['Third', 'tag: x'],
  ])
    await backend.call('save_query', { name, query })
  await backend.open('/saved/Second')
  const names = () => saved(page).locator('ul.saved a')
  await expect(names()).toHaveText(['First', 'Second', 'Third'])

  await page.getByTestId('saved-edit-open').click()
  const form = page.getByTestId('saved-edit')
  await expect(form.getByTestId('saved-text')).toHaveValue('task: done')
  await form.getByTestId('saved-text').fill('task: done\npath: work')
  await form.getByTestId('saved-save').click()
  await expect(form).toHaveCount(0)
  // it is in the graph's settings (the file holds the text in TOML's own way, so ask for the list)
  await expect
    .poll(async () => (await backend.call<{ name: string; query: string }[]>('list_saved_queries'))[1].query)
    .toBe('task: done\npath: work')
  await expect(names()).toHaveText(['First', 'Second', 'Third'])

  // a new name: the list keeps the place, the address follows
  await page.getByTestId('saved-edit-open').click()
  await page.getByTestId('saved-name').fill('Renamed')
  await page.keyboard.press('Control+Enter')
  await expect(page).toHaveURL(/#\/saved\/Renamed$/)
  await expect(names()).toHaveText(['First', 'Renamed', 'Third'])
  await expect(page.getByRole('heading', { name: 'Renamed' })).toBeVisible()
})

test('the edit form refuses a name that is taken and a query that does not parse, and Esc cancels', async ({
  backend,
  page,
}) => {
  await backend.call('save_query', { name: 'One', query: 'task: todo' })
  await backend.call('save_query', { name: 'Two', query: 'task: done' })
  await backend.open('/saved/Two')
  await page.getByTestId('saved-edit-open').click()
  await page.getByTestId('saved-name').fill('One')
  await page.getByTestId('saved-save').click()
  await expect(page.getByRole('alert')).toContainText('exists')
  await page.getByTestId('saved-name').fill('Two')
  await page.getByTestId('saved-text').fill('bogus: (((')
  await page.getByTestId('saved-save').click()
  await expect(page.getByRole('alert')).toBeVisible()
  expect((await backend.call<{ query: string }[]>('list_saved_queries')).map((q) => q.query)).toEqual([
    'task: todo',
    'task: done',
  ])
  await page.getByTestId('saved-text').press('Escape')
  await expect(page.getByTestId('saved-edit')).toHaveCount(0)
  await expect(page.getByTestId('saved-edit-open')).toBeVisible()
})
