import { expect, test } from './fixtures'

// Page-valued properties: picking a page for a page property, and filtering by it in a query.
test('a page property takes a page: picked from the list, written as a quoted link, and the page opens', async ({
  backend,
  page,
}) => {
  backend.write('pages/Acme.md', '- acme notes\n')
  backend.write('pages/part. Capgemini.md', '- partner page\n')
  await backend.open('/page/Acme')
  await page.getByTestId('props-toggle').click()
  await page.getByLabel('New property name').fill('partner')
  const value = page.getByRole('combobox', { name: 'New property value' })
  await value.fill('Capg')
  const option = page.getByRole('option', { name: /part\. Capgemini/ })
  await expect(option).toBeVisible()
  await expect(option).toContainText('page')
  await option.click()
  await expect(value).toHaveValue('[[part. Capgemini]]')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  // quoted: unquoted `[[x]]` would be a YAML list in a list
  await expect.poll(() => backend.read('pages/Acme.md')).toContain('partner: "[[part. Capgemini]]"')
  // the value shows without quotes and links to the page
  await expect(page.getByRole('combobox', { name: 'partner' })).toHaveValue('[[part. Capgemini]]')
  await page.getByTestId('prop-page-link').click()
  await expect(page).toHaveURL(/#\/page\/part\.%20Capgemini|#\/page\/part\. Capgemini/)
})

test('an existing page property is changed to another page from the list', async ({ backend, page }) => {
  backend.write('pages/Acme.md', '---\npartner: old\n---\n- acme\n')
  backend.write('pages/Beta Corp.md', '- b\n')
  await backend.open('/page/Acme')
  await page.getByTestId('props-toggle').click()
  const field = page.getByRole('combobox', { name: 'partner' })
  await field.fill('Beta')
  await page.getByRole('option', { name: /Beta Corp/ }).click()
  await expect.poll(() => backend.read('pages/Acme.md')).toContain('partner: "[[Beta Corp]]"')
})

test('a query filters by a page-valued property and offers pages for the value', async ({
  backend,
  page,
}) => {
  backend.write('pages/A.md', '---\npartner: "[[part. Capgemini]]"\n---\n- a\n')
  backend.write('pages/B.md', '---\npartner: [[part. Capgemini]]\n---\n- b\n') // written without quotes
  backend.write('pages/C.md', '---\npartner: "[[Other]]"\n---\n- c\n')
  backend.write('pages/part. Capgemini.md', '- partner\n')
  backend.write('pages/q.md', '- ```query\n  type: page\n  prop.partner:\n  ```\n')
  await backend.open('/page/q')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByText('click here to edit the query').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Control+End')
  await page.keyboard.press('ArrowUp')
  await page.keyboard.press('End')
  await page.keyboard.type(' Capg')
  const tip = page.locator('.cm-tooltip-autocomplete')
  await expect(tip).toContainText('part. Capgemini', { timeout: 5000 })
  await expect(tip).toContainText('page')
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await tip.getByText('part. Capgemini').last().click()
  await expect(page.getByTestId('block-editor')).toContainText('prop.partner: [[part. Capgemini]]')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/q.md')).toContain('prop.partner: [[part. Capgemini]]')
  const q = blocks.getByTestId('query-block').first()
  await expect(q.getByTestId('query-count')).toHaveText('2 results')
  await expect(q).toContainText('A')
  await expect(q).toContainText('B')
  await expect(q).not.toContainText('Other')
})
