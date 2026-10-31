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
  // one entry opens the list, whatever the number of templates
  const tip = page.locator('.cm-tooltip-autocomplete')
  await expect(tip).toContainText('Template…', { timeout: 5000 })
  await expect(tip).not.toContainText('Template: person')
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await page.keyboard.press('Enter')
  const list = page.getByTestId('template-picker')
  await expect(list.getByRole('option', { name: 'person' })).toBeVisible()
  await expect(page.getByLabel('Filter templates')).toBeFocused()
  await page.keyboard.press('Enter')
  await expect
    .poll(readOr(backend.read, 'pages/Meeting.md'))
    .toBe('---\ntype: person\nimportance: 1\n---\n\n- note\n- Contact Meeting\n  - Phone\n')
})

// PR-10: after /template the cursor is in the inserted block: at {{cursor}}, else at the end of the first one.
test('/template continues editing at {{cursor}}, else in the first inserted block', async ({
  backend,
  page,
}) => {
  backend.write(
    'templates/snippets/Call.md',
    '- Call {{date}} follow up {{date+7d}} ({{type}}/{{prop.partner}})\n  - With: {{cursor}} and team\n  - Notes\n',
  )
  backend.write('templates/snippets/Plain.md', '- Plain item\n  - child\n')
  backend.write('pages/Calls.md', '---\ntype: client\npartner: ACME\n---\n\n- first\n- \n')
  await backend.open('/page/Calls')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  const start = async (text: string, which: number) => {
    await blocks.getByText(text, { exact: true }).click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    await page.keyboard.press('End')
    await page.keyboard.type(' /template')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Template…', { timeout: 5000 })
    await page.waitForTimeout(150)
    await page.keyboard.press('Enter')
    await page.getByTestId('template-picker').getByRole('option').nth(which).click()
  }
  // {{cursor}}: typing lands there
  await start('first', 0)
  // the editor moves to the inserted block (it holds the text around the cursor)
  await expect(page.getByTestId('block-editor')).toContainText('With:', { timeout: 5000 })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('Ann')
  await page.keyboard.press('Escape')
  await expect
    .poll(readOr(backend.read, 'pages/Calls.md'))
    .toMatch(
      /- first\n- Call \d{4}-\d\d-\d\d follow up \d{4}-\d\d-\d\d \(client\/ACME\)\n {2}- With: Ann and team\n {2}- Notes\n- \n/,
    )
  // no marker: the first inserted block, at its end
  await start('Notes', 1)
  await expect(page.getByTestId('block-editor')).toContainText('Plain item', { timeout: 5000 })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('!')
  await page.keyboard.press('Escape')
  await expect
    .poll(readOr(backend.read, 'pages/Calls.md'))
    .toMatch(/ {2}- Notes\n {2}- Plain item!\n {4}- child\n/)
})

// `{{selection}}` is the text typed in the block besides the command, `{{clipboard}}` what was copied.
test('/template fills {{selection}} and {{clipboard}}', async ({ backend, page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  backend.write('templates/snippets/Mail.md', '- Mail to {{selection}}: {{clipboard}}{{cursor}}\n')
  backend.write('pages/Mails.md', '- Anna Nowak\n')
  await backend.open('/page/Mails')
  await page.evaluate(() => navigator.clipboard.writeText('see attachment'))
  await page.getByRole('tree', { name: 'Blocks' }).getByText('Anna Nowak', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /template')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Template…', { timeout: 5000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await page.getByTestId('template-picker').getByRole('option', { name: 'Mail' }).click()
  await expect(page.getByTestId('block-editor')).toContainText('Mail to', { timeout: 5000 })
  await page.keyboard.type('!')
  await page.keyboard.press('Escape')
  await expect
    .poll(readOr(backend.read, 'pages/Mails.md'))
    .toBe('- Anna Nowak\n- Mail to Anna Nowak: see attachment!\n')
})

// `{{ask:Label}}` and `{{ask:Label|default}}` ask before the blocks are inserted.
test('/template asks the questions of a template; Cancel inserts nothing', async ({ backend, page }) => {
  backend.write(
    'templates/snippets/Meeting.md',
    '- Meeting with {{ask:Who}} about {{ask:Topic|status}}\n  - {{ask:Who}}: {{cursor}}\n',
  )
  backend.write('pages/Asks.md', '- start\n')
  await backend.open('/page/Asks')
  const insert = async () => {
    await page.getByRole('tree', { name: 'Blocks' }).getByText('start', { exact: true }).click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    await page.keyboard.press('End')
    await page.keyboard.type(' /template')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Template…', { timeout: 5000 })
    await page.waitForTimeout(150)
    await page.keyboard.press('Enter')
    await page.getByTestId('template-picker').getByRole('option', { name: 'Meeting' }).click()
  }
  // cancelled: the page is unchanged and the block can be edited
  await insert()
  const dialog = page.getByTestId('template-ask')
  await expect(dialog.getByLabel('Who')).toBeFocused()
  await expect(dialog.getByLabel('Topic')).toHaveValue('status')
  await page.keyboard.press('Escape')
  await expect(dialog).toHaveCount(0)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  expect(backend.read('pages/Asks.md')).toBe('- start\n')
  // answered: one question asked once, used twice; the default is the starting text
  await insert()
  await dialog.getByLabel('Who').fill('Anna')
  await dialog.getByLabel('Topic').fill('budget')
  await dialog.getByRole('button', { name: 'Insert' }).click()
  await expect(page.getByTestId('block-editor')).toContainText('Anna:', { timeout: 5000 })
  await page.keyboard.type('hello')
  await page.keyboard.press('Escape')
  await expect
    .poll(readOr(backend.read, 'pages/Asks.md'))
    .toBe('- start\n- Meeting with Anna about budget\n  - Anna: hello\n')
})

// {{askpage:Label}}: the answer is a page, picked from the existing ones or a new name, and is inserted as a link.
test('{{askpage:…}} picks an existing page or takes a new name, as a link', async ({ backend, page }) => {
  backend.write('templates/snippets/Call.md', '- Call {{askpage:Partner}}\n')
  backend.write('pages/people/Eva.md', '- hi\n')
  backend.write('pages/Pg.md', '- start\n')
  await backend.open('/page/Pg')
  const insert = async () => {
    await page.getByRole('tree', { name: 'Blocks' }).getByText('start', { exact: true }).click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    await page.keyboard.press('End')
    await page.keyboard.type(' /template')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Template…', { timeout: 5000 })
    await page.waitForTimeout(150)
    await page.keyboard.press('Enter')
    await page.getByTestId('template-picker').getByRole('option', { name: 'Call' }).click()
  }
  const dialog = page.getByTestId('template-ask')
  // an existing page: type a part of the name, pick it from the list, then Insert
  await insert()
  await dialog.getByLabel('Partner').fill('Eva')
  await dialog.getByRole('option', { name: /people\/Eva/ }).click()
  await expect(dialog.getByLabel('Partner')).toHaveValue('people/Eva')
  await dialog.getByRole('button', { name: 'Insert' }).click()
  await expect(page.getByTestId('block-editor')).toBeVisible()
  await page.keyboard.press('Escape')
  await expect.poll(readOr(backend.read, 'pages/Pg.md')).toBe('- start\n- Call [[people/Eva]]\n')
  // a name that is no page yet is offered as a new page and stays as typed
  await insert()
  await dialog.getByLabel('Partner').fill('Zed Newname')
  await expect(dialog.getByRole('option', { name: /New page/ })).toBeVisible()
  await dialog.getByRole('button', { name: 'Insert' }).click()
  await expect(page.getByTestId('block-editor')).toBeVisible()
  await page.keyboard.press('Escape')
  await expect
    .poll(readOr(backend.read, 'pages/Pg.md'))
    .toBe('- start\n- Call [[Zed Newname]]\n- Call [[people/Eva]]\n')
})

// In an empty block the template takes the block's place; the cursor still goes to {{cursor}} in it.
test('/template in an empty block fills it and continues at {{cursor}}', async ({ backend, page }) => {
  backend.write('templates/snippets/Meet.md', '- Meet {{cursor}} today\n  - Notes\n')
  backend.write('pages/Empty.md', '- \n')
  await backend.open('/page/Empty')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('treeitem').first().click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('/template')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Template…', { timeout: 5000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await page.getByTestId('template-picker').getByRole('option', { name: 'Meet' }).click()
  await expect(page.getByTestId('block-editor')).toContainText('Meet', { timeout: 5000 })
  await page.keyboard.type('Capgemini')
  await page.keyboard.press('Escape')
  await expect.poll(readOr(backend.read, 'pages/Empty.md')).toBe('- Meet Capgemini today\n  - Notes\n')
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
  await expect.poll(readOr(backend.read, 'templates/person.md')).toContain('  - Mail\n')
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
  await expect.poll(readOr(backend.read, 'templates/place.md')).toBe('---\ntype: place\n---\n')
  const item = pane.getByTestId('template-list').getByRole('listitem').filter({ hasText: 'place' })
  const box = item.getByRole('textbox', { name: 'Text of the template place' })
  await expect(box).toHaveValue('---\ntype: place\n---\n')
  await box.fill('---\ntype: place\ncountry:\n---\n- About {{title}}\n')
  await item.getByTestId('template-save').click()
  await expect(item).toContainText('country')
  await expect(pane.getByTestId('new-template-form')).toHaveCount(0)
})

// PR-10: blocks saved with "Save as template…" are snippets in templates/snippets/, offered by /template.
test('selected blocks are saved as a template and inserted with /template', async ({ backend, page }) => {
  backend.write('pages/Source.md', '- Agenda for {{title}}\n  - Notes\n- Actions\n')
  backend.write('pages/Target.md', '- empty\n')
  await backend.open('/page/Source')
  await page.waitForTimeout(500) // the watcher reports the test's own write once and the page reloads
  const tree = page.getByRole('tree', { name: 'Blocks' })
  await expect(async () => {
    await page.keyboard.press('Escape')
    await tree.getByText('Agenda for', { exact: false }).first().click()
    await expect(page.getByTestId('block-editor')).toBeFocused({ timeout: 1500 })
    await page.keyboard.press('Escape')
    await expect(tree.locator('[aria-selected="true"]')).toHaveCount(1, { timeout: 1500 })
  }).toPass({ timeout: 10000 })

  // from the command palette; the block's children come along
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Save as template')
  await page.keyboard.press('Enter')
  const dlg = page.getByTestId('snippet-dialog')
  await expect(dlg.getByLabel('Blocks to save')).toContainText('Notes')
  await dlg.getByTestId('snippet-name').fill('Meeting')
  await dlg.getByTestId('snippet-save').click()
  await expect
    .poll(readOr(backend.read, 'templates/snippets/Meeting.md'))
    .toBe('- Agenda for {{title}}\n  - Notes\n')

  // a name that is taken offers to replace it
  await expect(dlg).toHaveCount(0)
  await tree.getByText('Actions', { exact: true }).click()
  await page.keyboard.press('Escape')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Save as template')
  await page.keyboard.press('Enter')
  await dlg.getByTestId('snippet-name').fill('meeting')
  await dlg.getByTestId('snippet-save').click()
  await expect(dlg.getByTestId('snippet-exists')).toBeVisible()
  await dlg.getByRole('button', { name: 'Cancel' }).click()

  // /template offers it on another page and inserts the blocks, with the page's name filled in
  await backend.open('/page/Target')
  await page.getByText('empty', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /template')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Template…', { timeout: 5000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  // saved blocks are their own group in the list; typing filters it
  const picker = page.getByTestId('template-picker')
  await expect(picker.getByRole('heading', { name: 'Saved blocks' })).toBeVisible()
  await page.keyboard.type('zzz')
  await expect(picker).toContainText('No template matches.')
  await page.keyboard.press('Control+a')
  await page.keyboard.type('meet')
  await picker.getByRole('option', { name: 'Meeting' }).click()
  await expect.poll(readOr(backend.read, 'pages/Target.md')).toBe('- empty\n- Agenda for Target\n  - Notes\n')

  // the settings list it; its text can be edited and it can be deleted
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Templates/ }).click()
  const pane = page.getByRole('tabpanel')
  const item = pane.getByTestId('snippet-list').getByRole('listitem').filter({ hasText: 'Meeting' })
  await item.getByRole('button', { name: 'Edit' }).click()
  const box = item.getByRole('textbox', { name: 'Text of the template Meeting' })
  await expect(box).toHaveValue('- Agenda for {{title}}\n  - Notes\n')
  await box.fill('- Agenda\n')
  await item.getByTestId('template-save').click()
  await expect.poll(readOr(backend.read, 'templates/snippets/Meeting.md')).toBe('- Agenda\n')
  await item.getByRole('button', { name: 'Delete' }).click()
  await expect(pane.getByTestId('no-snippets')).toBeVisible()
  await expect(pane.getByTestId('deleted-templates')).toContainText('saved blocks')
})
