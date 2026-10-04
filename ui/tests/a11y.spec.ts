import AxeBuilder from '@axe-core/playwright'
import { expect, test } from './fixtures'

test('no critical accessibility violations on the page view and the search dialog', async ({
  backend,
  page,
}) => {
  await backend.open('/page/work%2Fprojects%2Falpha')
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
  let r = await new AxeBuilder({ page }).analyze()
  expect(
    r.violations
      .filter((v) => v.impact === 'critical' || v.impact === 'serious')
      .map((v) => `${v.id}: ${v.nodes[0].html.slice(0, 100)}`),
  ).toEqual([])
  await page.keyboard.press('Control+Shift+F')
  await expect(page.getByRole('dialog', { name: 'Search' })).toBeVisible()
  r = await new AxeBuilder({ page }).analyze()
  expect(
    r.violations
      .filter((v) => v.impact === 'critical' || v.impact === 'serious')
      .map((v) => `${v.id}: ${v.nodes[0].html.slice(0, 100)}`),
  ).toEqual([])
})

test('keyboard-only flow: search a page, edit a block, follow a link back', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+Shift+F')
  await page.getByLabel('Search text').fill('alpha')
  await page.keyboard.press('Enter')
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
  await expect(async () => {
    await page.getByText('Notes from').click({ position: { x: 5, y: 5 } })
    await expect(page.getByTestId('block-editor')).toBeFocused({ timeout: 800 })
  }).toPass()
  await page.keyboard.press('End')
  await page.keyboard.type(' ok')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/work/projects/alpha.md')).toContain('see ((k3x9qa)) ok')
  await expect(page.getByTestId('save-status')).toHaveAttribute('data-status', 'saved')
})
