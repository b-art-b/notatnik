import { expect, test } from './fixtures'

const readOr = (read: (p: string) => string, rel: string) => () => {
  try {
    return read(rel)
  } catch {
    return ''
  }
}

const PERSON = '---\ntype: person\nimportance: 1\n---\n- Contact {{title}}\n  - Phone\n'

// NS-10, JR-6, PR-10: templates of a namespace and of journal days, set in the settings.
test('a namespace template and the journal template start new pages', async ({ backend, page }) => {
  backend.write('templates/person.md', PERSON)
  backend.write('templates/journal.md', '---\nmood:\n---\n- Plan for {{date}}\n')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Templates/ }).click()
  const pane = page.getByRole('tabpanel')
  await pane.getByLabel('Journal template').selectOption('journal')
  await pane.getByRole('button', { name: 'Add a namespace template' }).click()
  const rows = pane.getByTestId('namespace-templates')
  await rows.getByLabel('Namespace', { exact: true }).fill('people')
  await rows.getByLabel('Namespace', { exact: true }).press('Tab')
  await rows.getByLabel('Template', { exact: true }).selectOption('person')
  await expect.poll(readOr(backend.read, '.notatnik/config.toml')).toContain('templates/person.md')
  expect(backend.read('.notatnik/config.toml')).toContain('templates/journal.md')

  // a new page below the namespace
  await backend.open('/page/people%2FZofia')
  await page.getByText('Click here to start writing').click()
  await page.keyboard.type('met at the fair')
  await expect.poll(readOr(backend.read, 'pages/people/Zofia.md')).toContain('met at the fair')
  const text = backend.read('pages/people/Zofia.md')
  expect(text).toContain('type: person\nimportance: 1\n')
  expect(text).toContain('- Contact Zofia\n  - Phone\n- met at the fair')
})

test('/template inserts the blocks of a template after the block', async ({ backend, page }) => {
  backend.write('templates/person.md', PERSON)
  backend.write('pages/Meeting.md', '- note\n')
  await backend.open('/page/Meeting')
  await page.getByText('note', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /template')
  const tip = page.locator('.cm-tooltip-autocomplete')
  await expect(tip).toContainText('Template: person', { timeout: 5000 })
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await page.keyboard.press('Enter')
  await expect
    .poll(() => backend.read('pages/Meeting.md'))
    .toBe('---\ntype: person\nimportance: 1\n---\n\n- note\n- Contact Meeting\n  - Phone\n')
})

// QV-12: the query variables are offered in a query block, and listed in the settings.
test('`{{` in a query offers the variables; the settings list the placeholders', async ({
  backend,
  page,
}) => {
  backend.write('pages/q.md', '- ```query\n  task: todo\n  ```\n')
  await backend.open('/page/q')
  await page
    .getByTestId('query-block')
    .locator('xpath=ancestor::div[contains(@class,"body")]')
    .click({ position: { x: 3, y: 3 } })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Control+End')
  await page.keyboard.press('ArrowUp')
  await page.keyboard.press('End')
  await page.keyboard.press('Enter')
  await page.keyboard.type('page: {{')
  const tip = page.locator('.cm-tooltip-autocomplete')
  await expect(tip).toContainText('{{page}}', { timeout: 5000 })
  await expect(tip).toContainText('the page this query is on')
  await expect(tip).toContainText('{{namespace}}')

  await backend.open('/settings')
  await page.getByRole('tab', { name: /Templates/ }).click()
  const table = page.getByTestId('placeholders')
  await expect(table).toContainText('{{page}}')
  await expect(table).toContainText('{{cursor}}')
})

// A template is shown and edited as text in Settings → Templates; pages made after the change start from it.
test('a template is opened as text, edited and saved', async ({ backend, page }) => {
  backend.write('templates/person.md', PERSON)
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Templates/ }).click()
  const pane = page.getByRole('tabpanel')
  const item = pane.getByTestId('template-list').getByRole('listitem').filter({ hasText: 'person' })
  await item.getByRole('button', { name: 'Edit' }).click()
  const box = item.getByRole('textbox', { name: 'Text of the template person' })
  // the file as it is, nothing hidden
  await expect(box).toHaveValue(PERSON)
  const save = item.getByTestId('template-save')
  await expect(save).toBeDisabled()

  await box.fill('---\ntype: person\nimportance: 1\nphone:\n---\n- Contact {{title}}\n  - Phone\n  - Mail\n')
  await expect(item.getByText('Unsaved changes')).toBeVisible()
  await save.click()
  await expect.poll(() => backend.read('templates/person.md')).toContain('  - Mail\n')
  // the list shows the new fields and blocks, and nothing is unsaved any more
  await expect(item).toContainText('importance, phone')
  await expect(item).toContainText('3 blocks')
  await expect(item.getByText('Unsaved changes')).toHaveCount(0)

  // closing the editor and opening it again shows the saved text
  await item.getByRole('button', { name: 'Close' }).first().click()
  await expect(item.getByTestId('template-editor')).toHaveCount(0)
  await item.getByRole('button', { name: 'Edit' }).click()
  await expect(item.getByRole('textbox', { name: 'Text of the template person' })).toHaveValue(
    '---\ntype: person\nimportance: 1\nphone:\n---\n- Contact {{title}}\n  - Phone\n  - Mail\n',
  )
})

test('a new template is made from a page type name and opens for editing', async ({ backend, page }) => {
  backend.write('templates/person.md', PERSON)
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Templates/ }).click()
  const pane = page.getByRole('tabpanel')
  await pane.getByTestId('new-template').click()
  const form = pane.getByTestId('new-template-form')
  await expect(form.getByRole('button', { name: 'Create' })).toBeDisabled()
  // a name that exists (any case) is refused with the reason
  await form.getByLabel('Page type').fill('Person')
  await form.getByRole('button', { name: 'Create' }).click()
  await expect(page.getByRole('alert').filter({ hasText: /exists already/ })).toBeVisible()
  await form.getByLabel('Page type').fill('place')
  await form.getByRole('button', { name: 'Create' }).click()

  // the file is there and its text is open for editing
  await expect.poll(() => backend.read('templates/place.md')).toBe('---\ntype: place\n---\n')
  const item = pane.getByTestId('template-list').getByRole('listitem').filter({ hasText: 'place' })
  const box = item.getByRole('textbox', { name: 'Text of the template place' })
  await expect(box).toHaveValue('---\ntype: place\n---\n')
  await box.fill('---\ntype: place\ncountry:\n---\n- About {{title}}\n')
  await item.getByTestId('template-save').click()
  await expect(item).toContainText('country')
  await expect(pane.getByTestId('new-template-form')).toHaveCount(0)
})
