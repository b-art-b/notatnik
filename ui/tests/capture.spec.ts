import { expect, test } from './fixtures'

// AN-5: the capture screen, `#/capture`; the Android widget opens it (docs/phases/android-widgets.md, AW1).
test.use({ viewport: { width: 390, height: 780 } }) // a phone

// the test backend's clock stands at 2026-10-03
const journal = (backend: { read: (p: string) => string }) => backend.read('journals/2026/2026-10-03.md')

test('the capture screen opens on the input; Add puts the line at the end of today’s journal', async ({
  backend,
  page,
}) => {
  await backend.open('/capture')
  const input = page.getByRole('textbox', { name: "Add to today's journal" })
  await expect(input).toBeFocused()
  await input.fill('buy bread')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect(page.getByText(/^Added to /)).toBeVisible()
  await expect(page.getByRole('region', { name: 'Capture' })).toHaveCount(0)
  expect(journal(backend)).toContain('- buy bread\n')
  // a second line goes after the first
  await page.keyboard.press('Control+k')
  await page
    .getByRole('dialog', { name: 'Command palette' })
    .getByLabel('Command or page')
    .fill('Capture a note')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  const again = page.getByRole('textbox', { name: "Add to today's journal" })
  await again.fill('second')
  await again.press('Control+Enter')
  await expect(page.getByRole('region', { name: 'Capture' })).toHaveCount(0)
  expect(journal(backend)).toMatch(/- buy bread\n(.|\n)*- second\n$/)
})

test('an empty line adds nothing and says so; Cancel writes nothing', async ({ backend, page }) => {
  const before = journal(backend)
  await backend.open('/capture')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect(page.getByRole('alert')).toHaveText('There is nothing to add.')
  await page.getByRole('textbox', { name: "Add to today's journal" }).fill('   ')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect(page.getByRole('alert')).toBeVisible()
  await page.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('region', { name: 'Capture' })).toHaveCount(0)
  expect(journal(backend)).toBe(before)
})
