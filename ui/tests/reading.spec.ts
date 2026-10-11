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

test('link brackets are hidden when rendered, shown while editing, and a device setting', async ({
  backend,
  page,
}) => {
  backend.write('pages/b.md', '- see [[Other page]] now\n')
  await backend.open('/page/b')
  const before = () =>
    page
      .locator('.page-ref[data-page="Other page"]')
      .first()
      .evaluate((el) => getComputedStyle(el, '::before').content)
  await expect(page.locator('.page-ref[data-page="Other page"]').first()).toBeVisible()
  expect(await before()).toMatch(/none|normal/) // hidden by default
  // editing shows the raw Markdown with the brackets
  await page.getByText('see').click()
  await expect(page.getByTestId('block-editor')).toContainText('[[Other page]]')
  await page.keyboard.press('Escape')
  // the setting turns the brackets on for rendered links
  await backend.open('/settings')
  await page.getByRole('switch', { name: /brackets around links/ }).check()
  await expect(page.getByText('Settings saved')).toBeVisible()
  await backend.open('/page/b')
  await expect(page.locator('.page-ref[data-page="Other page"]').first()).toBeVisible()
  expect(await before()).toBe('"[["')
})

test('date language and format are device settings and change journal headings', async ({
  backend,
  page,
}) => {
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Dates & language' }).click()
  await page.getByLabel('Language', { exact: true }).selectOption('pl-PL')
  await expect(page.getByTestId('date-preview')).toContainText('sob., 3 paź 2026')
  await page.getByRole('radio', { name: /Day, full month, year/ }).check()
  await expect(page.getByTestId('date-preview')).toHaveText('3 października 2026')
  await backend.open('/journals')
  await expect(page.getByRole('heading', { name: '3 października 2026' })).toBeVisible()
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Dates & language' }).click()
  await page.getByRole('radio', { name: /ISO 8601/ }).check()
  await backend.open('/journals')
  await expect(page.getByRole('heading', { name: '2026-10-03' }).first()).toBeVisible()
})

test('a custom date pattern from settings is used for journal headings', async ({ backend, page }) => {
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Dates & language' }).click()
  await page.getByRole('radio', { name: /Custom pattern/ }).check()
  await page.getByRole('textbox', { name: 'Pattern' }).fill('[Day] D/M/YY, dddd')
  await page.getByRole('textbox', { name: 'Pattern' }).blur()
  await expect(page.getByTestId('date-preview')).toHaveText('Day 3/10/26, Saturday')
  await backend.open('/journals')
  await expect(page.getByRole('heading', { name: 'Day 3/10/26, Saturday' })).toBeVisible()
})
