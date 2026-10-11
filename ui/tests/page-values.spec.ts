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

// QV-12: `{{page}}` in a query is the page the query sits on, so the same block works on every partner page.
test('`{{page}}` in a query means the page the block is on', async ({ backend, page }) => {
  backend.write('pages/John Doe.md', '---\ntype: person\npartner: "[[part. Capgemini]]"\n---\n- a\n')
  backend.write('pages/Jane Roe.md', '---\ntype: person\npartner: "[[part. EY]]"\n---\n- b\n')
  const q = '- ```query\n  type: page\n  prop.type: person\n  prop.partner: {{page}}\n  ```\n'
  backend.write('pages/part. Capgemini.md', q)
  backend.write('pages/part. EY.md', q)
  await backend.open('/page/part. Capgemini')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await expect(blocks.getByTestId('query-count')).toHaveText('1 result')
  await expect(blocks.getByText('John Doe').first()).toBeVisible()
  await expect(blocks.getByText('Jane Roe')).toHaveCount(0)
  await backend.open('/page/part. EY')
  await expect(blocks.getByText('Jane Roe').first()).toBeVisible()
  await expect(blocks.getByText('John Doe')).toHaveCount(0)
})

// PR-10: setting `type` adds the properties of templates/<type>.md; the button adds fields added to it later.
test('setting a type adds the properties of its template, and the button adds new ones later', async ({
  backend,
  page,
}) => {
  backend.write('templates/person.md', '---\ntype: person\nimportance: 1\npartner:\n---\n- Contact\n')
  backend.write('pages/Dawid Benski.md', '')
  await backend.open('/page/Dawid Benski')
  await page.getByTestId('props-toggle').click()
  await page.getByLabel('New property name').fill('type')
  await page.getByRole('combobox', { name: 'New property value' }).fill('person')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect.poll(() => backend.read('pages/Dawid Benski.md')).toContain('importance: 1')
  const text = backend.read('pages/Dawid Benski.md')
  expect(text).toContain('partner:\n')
  expect(text).toContain('- Contact')
  await expect(page.getByRole('combobox', { name: 'importance' })).toHaveValue('1')
  await expect(page.getByTestId('apply-template')).toHaveCount(0)
  // a field added to the template later: offered as a button, written once
  backend.write('templates/person.md', '---\ntype: person\nimportance: 1\npartner:\nemail:\n---\n- Contact\n')
  await page.reload()
  await page.getByTestId('props-toggle').click()
  const btn = page.getByTestId('apply-template')
  await expect(btn).toContainText('email')
  await btn.click()
  await expect.poll(() => backend.read('pages/Dawid Benski.md')).toContain('email:')
  await expect(btn).toHaveCount(0)
  expect(backend.read('pages/Dawid Benski.md').match(/importance:/g)).toHaveLength(1)
})

// PR-10: a page becomes the template of its type from the app; the list is in the settings.
const tpl = (b: { read(p: string): string }) => () => {
  try {
    return b.read('templates/person.md')
  } catch {
    return ''
  }
}

test('save a page as the template of its type, replace it, and delete it in the settings', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/Model Person.md',
    '---\ntype: person\nimportance: 3\npartner: "[[part. Capgemini]]"\n---\n- Contact\n  - Phone\n',
  )
  await backend.open('/page/Model Person')
  await page.getByTestId('props-toggle').click()
  await page.getByTestId('save-as-template').click()
  const panel = page.getByTestId('save-template')
  await panel.getByRole('radio', { name: /Fields empty/ }).check()
  await panel.getByRole('button', { name: 'Save', exact: true }).click()
  await expect
    .poll(tpl(backend))
    .toBe('---\ntype: person\nimportance:\npartner:\n---\n- Contact\n  - Phone\n')
  // again: it asks before replacing
  await page.getByTestId('save-as-template').click()
  await panel.getByRole('radio', { name: /Keep these values/ }).check()
  await panel.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(panel.getByRole('alert')).toContainText('exists')
  expect(backend.read('templates/person.md')).toContain('importance:\n')
  await panel.getByRole('button', { name: 'Replace it' }).click()
  await expect.poll(tpl(backend)).toContain('importance: 3')
  // the settings list it; deleting asks first
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Templates/ }).click()
  const list = page.getByTestId('template-list')
  await expect(list).toContainText('person')
  await expect(list).toContainText('importance, partner')
  await list.getByRole('button', { name: 'Delete', exact: true }).click()
  await expect(page.getByTestId('no-templates')).toBeVisible()
  expect(tpl(backend)()).toBe('')
  // a deleted template can be restored
  const deleted = page.getByTestId('deleted-templates')
  await expect(deleted).toContainText('person')
  await deleted.getByRole('button', { name: 'Restore' }).click()
  await expect(page.getByTestId('template-list')).toContainText('person')
  await expect.poll(tpl(backend)).toContain('importance: 3')
})
