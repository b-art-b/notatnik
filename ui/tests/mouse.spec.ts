import { expect, test } from './fixtures'

test('mouse back/forward buttons and Alt+arrows navigate the history like Firefox', async ({
  backend,
  page,
}) => {
  backend.write('pages/one.md', '- first page\n')
  backend.write('pages/two.md', '- second page\n')
  await backend.open('/page/one')
  await page.getByRole('link', { name: 'two', exact: true }).first().click()
  await expect(page).toHaveURL(/#\/page\/two/)
  // back button = mouse button 3 in the DOM (Playwright's mouse API can't press it, so send the events)
  const press = (button: number) =>
    page.evaluate((b) => {
      for (const type of ['mousedown', 'mouseup']) {
        document.body.dispatchEvent(new MouseEvent(type, { button: b, bubbles: true, cancelable: true }))
      }
    }, button)
  await press(3)
  await expect(page).toHaveURL(/#\/page\/one/)
  await press(4) // forward button
  await expect(page).toHaveURL(/#\/page\/two/)
  await press(3)
  await expect(page).toHaveURL(/#\/page\/one/)
  // forward with Alt+→ (focus is not in a text field)
  await page.locator('body').click({ position: { x: 5, y: 5 } })
  await page.keyboard.press('Alt+ArrowRight')
  await expect(page).toHaveURL(/#\/page\/two/)
  await page.keyboard.press('Alt+ArrowLeft')
  await expect(page).toHaveURL(/#\/page\/one/)
})

test('Alt+arrows inside the editor keep editing the text instead of navigating', async ({
  backend,
  page,
}) => {
  backend.write('pages/one.md', '- some words here\n')
  await backend.open('/page/one')
  await page.getByText('some words').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  const url = page.url()
  await page.keyboard.press('Alt+ArrowLeft')
  expect(page.url()).toBe(url)
  await expect(page.getByTestId('block-editor')).toBeFocused()
})

test('middle click and Ctrl+click on a page link open it in the right sidebar', async ({ backend, page }) => {
  backend.write('pages/one.md', '- see [[two]] and [[three]]\n')
  backend.write('pages/two.md', '- second page body\n')
  backend.write('pages/three.md', '- third page body\n')
  await backend.open('/page/one')
  const url = page.url()
  await page.locator('a.page-ref[data-page="two"]').click({ button: 'middle' })
  await expect(page.getByText('second page body')).toBeVisible()
  await page.locator('a.page-ref[data-page="three"]').click({ modifiers: ['Control'] })
  await expect(page.getByText('third page body')).toBeVisible()
  expect(page.url()).toBe(url) // the main page did not change
})

test('the journals feed: every day header, today included, opens the day as a page', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  const today = page.locator('[data-testid="journals"] a.title').first()
  await expect(today).toBeVisible()
  await today.click()
  await expect(page).toHaveURL(/#\/page\/2026-10-03/)
})

test('the sidebar can be hidden and shown again, and stays hidden after a reload', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  const sidebar = page.getByRole('complementary', { name: 'Sidebar' })
  await expect(sidebar).toBeVisible()
  await page.getByTestId('left-toggle').click()
  await expect(sidebar).toBeHidden()
  await page.reload()
  await expect(sidebar).toBeHidden()
  await page.keyboard.press('Control+\\')
  await expect(sidebar).toBeVisible()
})

test('the sidebar can be resized by dragging its edge or with the keyboard, and the width is remembered', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  const sidebar = page.getByRole('complementary', { name: 'Sidebar' })
  const width = async () => Math.round((await sidebar.boundingBox())!.width)
  expect(await width()).toBe(240)
  const handle = page.getByTestId('left-resizer')
  const box = (await handle.boundingBox())!
  await page.mouse.move(box.x + box.width / 2, box.y + 200)
  await page.mouse.down()
  await page.mouse.move(380, box.y + 200, { steps: 5 })
  await page.mouse.up()
  expect(await width()).toBe(380)
  await page.reload()
  expect(await width()).toBe(380)
  // keyboard: arrows move it, Home resets
  await handle.focus()
  await page.keyboard.press('ArrowLeft')
  expect(await width()).toBe(370)
  await page.keyboard.press('Home')
  expect(await width()).toBe(240)
  // limits
  await page.mouse.move(240, 300)
  const b2 = (await handle.boundingBox())!
  await page.mouse.move(b2.x + 3, 300)
  await page.mouse.down()
  await page.mouse.move(20, 300, { steps: 4 })
  await page.mouse.up()
  expect(await width()).toBe(180)
})

test('the sidebar width can be set in Settings', async ({ backend, page }) => {
  await backend.open('/settings')
  const sidebar = page.getByRole('complementary', { name: 'Sidebar' })
  expect(Math.round((await sidebar.boundingBox())!.width)).toBe(240)
  await page.getByLabel('Sidebar width').fill('320')
  await expect(page.getByTestId('left-width')).toHaveText('320px')
  expect(Math.round((await sidebar.boundingBox())!.width)).toBe(320)
  await page.getByRole('button', { name: 'Reset' }).last().click()
  expect(Math.round((await sidebar.boundingBox())!.width)).toBe(240)
})
