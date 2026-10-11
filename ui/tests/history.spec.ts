import { expect, test } from './fixtures'

// VH-1..VH-3: the page history under the page title.
test('history lists the versions of a page, shows the difference and restores one', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- original\n')
  await backend.open('/page/t')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByText('original').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' and more')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- original and more\n')

  await page.getByTestId('history-toggle').click()
  const dialog = page.getByTestId('history-dialog')
  const versions = dialog.getByTestId('history-version')
  await expect(versions).toHaveCount(2) // the original and the edit
  await versions.last().click()
  const diff = dialog.getByTestId('history-diff')
  await expect(diff.locator('[data-kind="del"]')).toContainText('original and more')
  await expect(diff.locator('[data-kind="add"]')).toContainText('- original')
  // the colours are real, not just classes
  const bg = await diff
    .locator('[data-kind="add"]')
    .first()
    .evaluate((el) => getComputedStyle(el).backgroundColor)
  expect(bg).not.toBe('rgba(0, 0, 0, 0)')

  await dialog.getByTestId('history-restore').click()
  await expect.poll(() => backend.read('pages/t.md')).toBe('- original\n')
  await expect(blocks.getByText('original', { exact: true })).toBeVisible()
  // what the restore replaced is a version now
  await expect(versions).toHaveCount(3)
})

test('journal days have a history too', async ({ backend, page }) => {
  backend.write('journals/2026/2026-10-01.md', '- day one\n')
  await backend.open('/page/2026-10-01')
  await expect(page.getByTestId('history-toggle')).toBeVisible()
  await expect(page.getByTestId('delete-page')).toHaveCount(0)
})
