import { expect, test } from './fixtures'

test('the left panel groups pages by a page property chosen in the picker', async ({ backend, page }) => {
  backend.write('pages/Ann.md', '---\ntype: person\n---\n- ann\n')
  backend.write('pages/Bob.md', '---\ntype: person\n---\n- bob\n')
  backend.write('pages/Acme.md', '---\ntype: client\n---\n- acme\n')
  backend.write('pages/Misc.md', '- no type here\n')
  await backend.open('/page/Misc')
  const groups = page.getByTestId('grouped-pages')
  await expect(groups).toHaveCount(0) // the namespace tree is the default
  await page.getByLabel('Group pages by').selectOption('type')
  await expect(groups).toBeVisible()
  const person = groups.locator('section[data-group="person"]')
  await expect(person.locator('.count')).toHaveText('2')
  await expect(person.locator('a[data-page="Ann"]')).toBeVisible()
  await expect(person.locator('a[data-page="Bob"]')).toBeVisible()
  await expect(groups.locator('section[data-group="client"] a[data-page="Acme"]')).toBeVisible()
  // pages without the property are in a folded group at the end
  const none = groups.locator('section[data-group=""]')
  await expect(none).toContainText('no type')
  await expect(none.locator('a[data-page="Misc"]')).toHaveCount(0)
  await none.getByRole('button').click()
  await expect(none.locator('a[data-page="Misc"]')).toBeVisible()
  // folding a group hides its pages; the choice survives a reload
  await person.getByRole('button').click()
  await expect(person.locator('a[data-page="Ann"]')).toHaveCount(0)
  await page.reload()
  await expect(page.getByLabel('Group pages by')).toHaveValue('type')
  await expect(groups.locator('section[data-group="person"] a[data-page="Ann"]')).toHaveCount(0)
  // back to namespaces
  await page.getByLabel('Group pages by').selectOption('')
  await expect(page.getByTestId('grouped-pages')).toHaveCount(0)
})

test('adding, changing and removing a page property from the page header updates the file and the groups', async ({
  backend,
  page,
}) => {
  backend.write('pages/Ann.md', '---\ntype: person\n---\n- ann\n')
  backend.write('pages/Misc.md', '- no type here\n')
  await backend.open('/page/Misc')
  await page.getByLabel('Group pages by').selectOption('type')
  await page.getByTestId('props-toggle').click()
  // the form must really be visible (Playwright counts `opacity: 0` as visible, so check the style)
  const form = page.locator('form.newprop')
  await expect(form).toBeVisible()
  expect(await form.evaluate((el) => getComputedStyle(el).opacity)).toBe('1')
  await expect(page.getByLabel('New property name')).toBeInViewport({ ratio: 0.9 })
  // add `type: client` to Misc: it creates the frontmatter, the file's blocks stay untouched
  await page.getByLabel('New property name').fill('type')
  await page.getByLabel('New property value').fill('client')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect.poll(() => backend.read('pages/Misc.md')).toBe('---\ntype: client\n---\n\n- no type here\n')
  const groups = page.getByTestId('grouped-pages')
  await expect(groups.locator('section[data-group="client"] a[data-page="Misc"]')).toBeVisible()
  // change the value: the page moves to another group
  const field = page.getByTestId('page-props').getByRole('combobox', { name: 'type' })
  await field.fill('person')
  await field.blur()
  await expect.poll(() => backend.read('pages/Misc.md')).toContain('type: person')
  await expect(groups.locator('section[data-group="person"] a[data-page="Misc"]')).toBeVisible()
  await expect(groups.locator('section[data-group="client"]')).toHaveCount(0)
  // a value that YAML could misread is quoted
  await page.getByLabel('New property name').fill('note')
  await page.getByLabel('New property value').fill('a: b #c')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect.poll(() => backend.read('pages/Misc.md')).toContain('note: "a: b #c"')
  // remove it again
  await page.getByRole('button', { name: 'Remove note' }).click()
  await expect.poll(() => backend.read('pages/Misc.md')).not.toContain('note:')
  // the typed key offers the values already in use
  await page.getByLabel('New property name').fill('type')
  await page.getByRole('combobox', { name: 'New property value' }).focus()
  await expect(page.getByRole('option', { name: /person/ })).toBeVisible()
})

test('properties can be added and edited on a journal page too', async ({ backend, page }) => {
  await backend.open('/page/2026-10-03')
  await page.getByTestId('props-toggle').click()
  await page.getByLabel('New property name').fill('mood')
  await page.getByLabel('New property value').fill('good')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect.poll(() => backend.read('journals/2026/2026-10-03.md')).toMatch(/^---\nmood: good\n---\n/)
  const field = page.getByTestId('page-props').getByRole('combobox', { name: 'mood' })
  await field.fill('great')
  await field.blur()
  await expect.poll(() => backend.read('journals/2026/2026-10-03.md')).toContain('mood: great')
})

test('settings: categories are tabs, changes apply at once and are confirmed', async ({ backend, page }) => {
  await backend.open('/settings')
  await expect(page.getByRole('tab', { name: 'Appearance' })).toHaveAttribute('aria-selected', 'true')
  // arrow keys move between the categories
  await page.getByRole('tab', { name: 'Appearance' }).focus()
  await page.keyboard.press('ArrowDown')
  await expect(page.getByRole('tab', { name: 'Dates & language' })).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByText('This device').first()).toBeVisible()
  await page.getByRole('tab', { name: 'Tasks' }).click()
  await expect(page.getByText('This graph').first()).toBeVisible()
  await page.getByLabel('Completion dates to keep').fill('3')
  await page.getByLabel('Completion dates to keep').blur()
  await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible()
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('completed_log_limit = 3')
  // the zoom stepper
  await page.getByRole('tab', { name: 'Appearance' }).click()
  await page.getByRole('button', { name: 'Zoom in' }).click()
  await expect(page.getByTestId('zoom-level')).toHaveText('110%')
  await page.getByRole('group', { name: 'Zoom' }).getByRole('button', { name: 'Reset' }).click()
  await expect(page.getByTestId('zoom-level')).toHaveText('100%')
  // the graph tab shows the folder
  await page.getByRole('tab', { name: 'Graph' }).click()
  await expect(page.getByTestId('graph-folder')).toContainText('graph')
})

test('clicking a property shows all the pages that have it', async ({ backend, page }) => {
  backend.write('pages/Ann.md', '---\ntype: person\nteams: [red, blue]\n---\n- ann\n')
  backend.write('pages/Bob.md', '---\ntype: Person\n---\n- bob\n')
  backend.write('pages/Acme.md', '---\ntype: client\n---\n- acme\n')
  await backend.open('/page/Ann')
  await page.getByTestId('props-toggle').click()
  const props = page.getByTestId('page-props')
  // the value link: only the pages with `type: person` (case-insensitive)
  await props.getByTestId('prop-value-link').first().click()
  await expect(page).toHaveURL(/#\/property\/type\?value=person/)
  await expect(page.getByTestId('property-count')).toHaveText('2')
  await expect(page.locator('a[data-page="Ann"]').last()).toBeVisible()
  await expect(page.locator('a[data-page="Bob"]').last()).toBeVisible()
  await expect(page.locator('main a[data-page="Acme"]')).toHaveCount(0)
  // the other values of the property are links too
  await page.locator('main a.chip', { hasText: 'client' }).click()
  await expect(page).toHaveURL(/value=client/)
  await expect(page.locator('main a[data-page="Acme"]')).toBeVisible()
  await expect(page.locator('main a[data-page="Ann"]')).toHaveCount(0)
  // the key link: every page that has the property, grouped by value
  await page.goBack()
  await page.goBack()
  await page.getByTestId('props-toggle').click()
  await props.getByTestId('prop-key-link').first().click()
  await expect(page).toHaveURL(/#\/property\/type$/)
  await expect(page.locator('main section[data-group="person"] a[data-page="Ann"]')).toBeVisible()
  await expect(page.locator('main section[data-group="client"] a[data-page="Acme"]')).toBeVisible()
  // a list value has one link per item
  await page.goBack()
  await page.getByTestId('props-toggle').click()
  await expect(props.getByTestId('prop-value-link').filter({ hasText: 'red' })).toBeVisible()
  await expect(props.getByTestId('prop-value-link').filter({ hasText: 'blue' })).toBeVisible()
  // opening a page from the list works
  await props.getByTestId('prop-value-link').filter({ hasText: 'red' }).click()
  await expect(page.locator('main a[data-page="Ann"]')).toBeVisible()
  await page.locator('main a[data-page="Ann"]').click()
  await expect(page).toHaveURL(/#\/page\/Ann/)
})

test('the journals landing page has no properties line, the day opened as a page has', async ({
  backend,
  page,
}) => {
  await backend.open('/journals')
  await expect(page.getByTestId('journals')).toBeVisible()
  await expect(page.getByTestId('props-toggle')).toHaveCount(0)
  await backend.open('/page/2026-10-03')
  await expect(page.getByTestId('props-toggle')).toBeVisible()
})
