import { expect, test } from './fixtures'

test('today journal renders and the page title is visible', async ({ backend, page }) => {
  await backend.open()
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Sat, 3 Oct 2026')
  await expect(page.getByText('Weekend plans')).toBeVisible()
})

test('page with frontmatter, tasks, refs renders', async ({ backend, page }) => {
  await backend.open('/page/work%2Fprojects%2Falpha')
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Project Alpha')
  await expect(page.locator('[data-task="TODO"]').first()).toBeVisible()
  await expect(page.locator('a.page-ref[data-page="people/anna"]').first()).toBeVisible()
  await expect(page.getByTestId('backlink-count')).toHaveText('2')
})
