import { expect, test } from './fixtures'

// PR-3 / PR-6: the Logseq-style property flow in the editor, the property card and the key/value view.

async function openBlock(page: import('@playwright/test').Page, text: string) {
  await page.getByRole('tree', { name: 'Blocks' }).getByText(text).first().click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
}

test('`::` on a new line offers known keys; a new key can be created; values suggest pages', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- first\n  client:: [[Acme]]\n- second\n')
  backend.write('pages/Acme.md', '- Acme page\n')
  await backend.open('/page/t')
  await openBlock(page, 'second')
  await page.keyboard.press('End')
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.type('::')
  const menu = page.locator('.cm-tooltip-autocomplete')
  await expect(menu).toContainText('client')
  // typing after the colons filters the keys
  await page.keyboard.type('cl')
  await expect(menu).not.toContainText('scheduled')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  // the key is written and the value menu opens right away
  await expect(page.getByTestId('block-editor')).toContainText('client::')
  await page.keyboard.type('Ac')
  await expect(menu).toContainText('page')
  // the value used before, then the page: pick the page row
  await menu.getByRole('option', { name: 'Acme page' }).click()
  await expect(page.getByTestId('block-editor')).toContainText('client:: [[Acme]]')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- first\n  client:: [[Acme]]\n- second\n  client:: [[Acme]]\n')
})

test('a key nobody uses yet gets a "Create property" row', async ({ backend, page }) => {
  backend.write('pages/t.md', '- first\n')
  await backend.open('/page/t')
  await openBlock(page, 'first')
  await page.keyboard.press('End')
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.type('reminder::')
  const menu = page.locator('.cm-tooltip-autocomplete')
  await expect(menu).toContainText('Create property: reminder')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('block-editor')).toContainText('reminder::')
  await page.keyboard.type('do something')
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- first\n  reminder:: do something\n')
})

test('/property starts a property line under the others', async ({ backend, page }) => {
  backend.write('pages/t.md', '- first\n  type:: note\n  some body\n')
  await backend.open('/page/t')
  await openBlock(page, 'first')
  await page.keyboard.press('Control+End')
  await page.keyboard.type(' /property')
  const menu = page.locator('.cm-tooltip-autocomplete')
  await expect(menu).toContainText('Property')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('block-editor')).toContainText('::')
  await page.keyboard.type('owner')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Create property: owner')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await page.keyboard.type('me')
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- first\n  type:: note\n  owner:: me\n  some body\n')
})

test('the property card edits, adds and removes single lines', async ({ backend, page }) => {
  backend.write('pages/t.md', '- first\n  type:: partner\n  client:: [[Acme]]\n- other\n')
  backend.write('pages/Acme.md', '- Acme page\n')
  await backend.open('/page/t')
  const card = page.getByRole('tree', { name: 'Blocks' }).getByTestId('props').first()
  await expect(card).toContainText('type:')
  await expect(card).toContainText('partner')
  await expect(card.locator('a.page-ref[data-page="Acme"]')).toBeVisible()
  // the key is bold: a card, not plain text
  const weight = await card
    .locator('.prop[data-key="type"] .key')
    .evaluate((e) => getComputedStyle(e).fontWeight)
  expect(Number(weight)).toBeGreaterThanOrEqual(600)
  // no empty row for "add": two properties are two rows
  const rows = await card.locator('.prop').count()
  const box = await card.boundingBox()
  expect(box!.height).toBeLessThan(rows * 30 + 20)
  // edit
  await card.locator('.prop[data-key="type"]').hover()
  await card.getByRole('button', { name: 'Edit type' }).click()
  await card.getByRole('combobox', { name: 'Value of type' }).fill('customer')
  await page.keyboard.press('Enter')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- first\n  type:: customer\n  client:: [[Acme]]\n- other\n')
  // add
  await card.hover()
  await card.getByRole('button', { name: 'Add property' }).click()
  await card.getByRole('combobox', { name: 'New property name' }).fill('Review Date')
  await card.getByRole('combobox', { name: 'New property value' }).fill('2026-11-01')
  await page.keyboard.press('Enter')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- first\n  type:: customer\n  client:: [[Acme]]\n  review-date:: 2026-11-01\n- other\n')
  // remove
  await card.locator('.prop[data-key="client"]').hover()
  await card.getByRole('button', { name: 'Remove client' }).click()
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- first\n  type:: customer\n  review-date:: 2026-11-01\n- other\n')
})

test('a key opens the view of its blocks; a value filters; children can be included', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/t.md',
    '- group\n  client:: Acme\n  - child task\n- elsewhere\n  client:: Beta\n- loose\n',
  )
  await backend.open('/page/t')
  const card = page.getByRole('tree', { name: 'Blocks' }).getByTestId('props').first()
  await card.getByRole('link', { name: 'client:' }).click()
  await expect(page).toHaveURL(/#\/block-property\/client$/)
  await expect(page.getByRole('heading', { level: 1 })).toContainText('client')
  await expect(page.getByText('elsewhere')).toBeVisible()
  await page.goto(page.url().replace(/#.*/, '') + '#/block-property/client?value=Acme')
  await expect(page.getByText('group')).toBeVisible()
  await expect(page.getByText('elsewhere')).toHaveCount(0)
  await expect(page.getByText('child task')).toHaveCount(0)
  await page.getByTestId('include-children').check()
  await expect(page.getByText('child task')).toBeVisible()
})

test('a block with the same property key twice shows both lines and keeps the app running', async ({
  backend,
  page,
}) => {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  // a second `/clip` into the same block writes `source::` twice
  backend.write('pages/t.md', '- note\n  source:: https://a.example/\n  source:: https://b.example/\n')
  await backend.open('/page/t')
  await expect(page.getByTestId('props').locator('.prop[data-key="source"]')).toHaveCount(2)
  expect(errors).toEqual([])
})
