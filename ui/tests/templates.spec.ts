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
  await expect.poll(() => backend.read('pages/Meeting.md')).toBe('- note\n- Contact Meeting\n  - Phone\n')
})
