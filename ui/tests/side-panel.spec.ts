import { expect, test } from './fixtures'

// Linked and unlinked references and the page's tasks can sit in a panel at the right instead of below the page.

test('references and tasks stay below the page until Settings moves them to the side panel', async ({
  backend,
  page,
}) => {
  backend.write('pages/target.md', '- TODO a task on the target\n')
  backend.write('pages/source.md', '- see [[target]] and a TODO\n- TODO call about target\n')
  await backend.open('/page/target')
  const article = page.locator('main')
  await expect(page.getByTestId('side-panel')).toHaveCount(0)
  await expect(article.getByRole('region', { name: 'Linked references', exact: true })).toBeVisible()

  // Settings → Appearance
  await page.getByRole('link', { name: 'Settings' }).click()
  await page.getByRole('radio', { name: 'Side panel' }).first().check() // references
  await page.getByRole('radio', { name: 'Side panel' }).nth(1).check() // tasks
  await page
    .getByRole('link', { name: 'target', exact: false })
    .first()
    .click()
    .catch(() => undefined)
  await backend.open('/page/target')
  const side = page.getByTestId('side-panel')
  await expect(side.getByRole('region', { name: 'Linked references', exact: true })).toBeVisible()
  await expect(side).toContainText('source')
  await expect(side.getByText('Tasks', { exact: false }).first()).toBeVisible()
  // no longer below the page
  await expect(article.getByRole('region', { name: 'Linked references', exact: true })).toHaveCount(0)

  // the panel hides and shows again; the choice survives a reload
  await page.getByTestId('right-toggle').click()
  await expect(page.getByTestId('side-panel')).toHaveCount(0)
  await page.reload()
  await expect(page.getByTestId('side-panel')).toHaveCount(0)
  await page.keyboard.press('Control+Shift+Backslash')
  await expect(page.getByTestId('side-panel')).toBeVisible()
})

test('a narrow window shows them below the page again', async ({ backend, page }) => {
  await page.addInitScript(() =>
    localStorage.setItem('notatnik-placement', JSON.stringify({ refs: 'side', tasks: 'side' })),
  )
  backend.write('pages/target.md', '- the target\n')
  backend.write('pages/source.md', '- see [[target]]\n')
  await page.setViewportSize({ width: 1400, height: 800 })
  await backend.open('/page/target')
  await expect(
    page.getByTestId('side-panel').getByRole('region', { name: 'Linked references', exact: true }),
  ).toBeVisible()
  await page.setViewportSize({ width: 900, height: 800 })
  await expect(page.getByTestId('side-panel')).toHaveCount(0)
  await expect(
    page.locator('main').getByRole('region', { name: 'Linked references', exact: true }),
  ).toBeVisible()
})

test('the side panel can be resized with the keyboard and reset', async ({ backend, page }) => {
  await page.addInitScript(() =>
    localStorage.setItem('notatnik-placement', JSON.stringify({ refs: 'side', tasks: 'bottom' })),
  )
  backend.write('pages/target.md', '- the target\n')
  backend.write('pages/source.md', '- see [[target]]\n')
  await backend.open('/page/target')
  const handle = page.getByTestId('right-resizer')
  await handle.focus()
  const width = () => page.getByTestId('side-panel').evaluate((e) => e.getBoundingClientRect().width)
  const before = await width()
  await page.keyboard.press('ArrowLeft')
  expect(await width()).toBeGreaterThan(before)
  await page.keyboard.press('Home')
  expect(Math.round(await width())).toBe(380)
})
