import { expect, test } from './fixtures'

const PAGE = (q: string) => `- Overview\n  - \`\`\`query\n    ${q.split('\n').join('\n    ')}\n    \`\`\`\n`

test.describe('queries fixture', () => {
  test.use({ fixture: 'queries' })

  test('a query block renders results with a count and updates when another page changes', async ({
    backend,
    page,
  }) => {
    backend.write('pages/dash.md', PAGE('tag: urgent\ntask: any'))
    await backend.open('/page/dash')
    await expect(page.getByTestId('query-count')).toHaveText('4 results')
    // add #urgent to a block on another page through the backend: the open query updates without reload
    const p = await backend.call<{ hash: string }>('get_page', { name: 'work/tasks' })
    await backend.call('apply_ops', {
      page: 'work/tasks',
      base_hash: p.hash,
      ops: [
        { op: 'update_text', block: { path: [1] }, text: 'TODO Today task #urgent\nscheduled:: 2026-10-03' },
      ],
    })
    await expect(page.getByTestId('query-count')).toHaveText('5 results', { timeout: 5000 })
  })

  test('query errors are shown inline and the page stays editable', async ({ backend, page }) => {
    backend.write('pages/bad.md', PAGE('tags: x') + '- another block\n')
    await backend.open('/page/bad')
    await expect(page.getByTestId('query-errors')).toContainText('unknown key `tags`; did you mean `tag`?')
    await page.getByText('another block').click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
  })

  test('table view: sort by a column and edit a cell writes one property line', async ({ backend, page }) => {
    backend.write(
      'pages/books.md',
      PAGE('page: reading/books\nview: table\ncolumns: text, prop.rating, prop.status'),
    )
    await backend.open('/page/books')
    await expect(page.getByTestId('query-table')).toBeVisible()
    await page.getByRole('button', { name: 'rating' }).click()
    const first = page.locator('[data-testid="query-table"] tbody tr').first()
    await expect(first).toContainText('Żółw')
    // edit the status cell of Solaris
    const row = page.locator('tr', { hasText: 'Solaris' })
    await row.locator('td').nth(2).dblclick()
    await page.getByLabel('Edit prop.status').fill('finished')
    await page.keyboard.press('Enter')
    await expect.poll(() => backend.read('pages/reading/books.md')).toContain('status:: finished')
    const lines = backend.read('pages/reading/books.md').split('\n')
    expect(lines.filter((l) => l.includes('finished')).length).toBe(1)
  })

  test('show query also shows the query with the placeholders replaced', async ({ backend, page }) => {
    backend.write('pages/dbg.md', PAGE('page: {{page}}\ntext: {{title}}'))
    await backend.open('/page/dbg')
    await page.getByRole('button', { name: 'show query' }).click()
    const shown = page.getByTestId('query-expanded')
    await expect(shown).toContainText('page: [[dbg]]')
    await expect(shown).toContainText('text: dbg')
    // a query without placeholders has nothing to add
    backend.write('pages/plain.md', PAGE('tag: urgent'))
    await backend.open('/page/plain')
    await page.getByRole('button', { name: 'show query' }).click()
    await expect(page.getByTestId('query-expanded')).toHaveCount(0)
  })

  test('table view: children: yes shows the sub-bullets under the text, without it only the block', async ({
    backend,
    page,
  }) => {
    backend.write('pages/notes.md', '- Parent block\n  - first detail\n  - second detail\n')
    backend.write('pages/q.md', PAGE('page: notes\ntext: Parent\nview: table'))
    await backend.open('/page/q')
    const table = page.getByTestId('query-table')
    await expect(table).toContainText('Parent block')
    await expect(table).not.toContainText('first detail')
    backend.write('pages/q.md', PAGE('page: notes\ntext: Parent\nview: table\nchildren: yes'))
    await backend.open('/page/q')
    await expect(table).toContainText('first detail')
    await expect(table).toContainText('second detail')
  })

  test("kanban: dragging a card changes only that task's state", async ({ backend, page }) => {
    backend.write(
      'pages/board.md',
      PAGE('task: todo, doing\nns: work/projects/alpha\ngroup: task\nview: kanban'),
    )
    await backend.open('/page/board')
    const before = backend.read('pages/work/projects/alpha.md')
    await expect(page.getByTestId('query-kanban')).toBeVisible()
    const card = page.locator('[data-card]', { hasText: 'Write spec' })
    await card.dragTo(page.locator('[data-col="doing"]'))
    await expect
      .poll(() => backend.read('pages/work/projects/alpha.md'))
      .toContain('- DOING Write spec #urgent')
    const after = backend.read('pages/work/projects/alpha.md')
    expect(after.replace('DOING Write spec', 'TODO Write spec')).toBe(before)
  })

  test('kanban by property: dragging changes the property line', async ({ backend, page }) => {
    backend.write('pages/board2.md', PAGE('page: reading/books\ngroup: prop.status\nview: kanban'))
    await backend.open('/page/board2')
    await page.locator('[data-card]', { hasText: 'Solaris' }).dragTo(page.locator('[data-col="done"]'))
    await expect.poll(() => backend.read('pages/reading/books.md')).toContain('rating:: 4\n  status:: done')
  })

  test('table with group: has a header row per group', async ({ backend, page }) => {
    backend.write('pages/tg.md', PAGE('page: reading/books\ngroup: prop.status\nview: table'))
    await backend.open('/page/tg')
    const table = page.getByTestId('query-table')
    await expect(table.locator('tr.group').first()).toBeVisible()
    expect(await table.locator('tr.group').count()).toBeGreaterThan(1)
    const rows = await table.locator('tr[data-row]').count()
    await table
      .getByRole('button', { name: /^Toggle / })
      .first()
      .click()
    expect(await table.locator('tr[data-row]').count()).toBeLessThan(rows)
  })

  test('timeline: tasks sit on their deadline; dragging moves the date by the dragged days', async ({
    backend,
    page,
  }) => {
    backend.write('pages/cal.md', PAGE('ns: work/projects/alpha\ntask: any\nview: timeline'))
    await backend.open('/page/cal')
    const ev = page.locator('[data-ev]', { hasText: 'Review design' })
    await expect(ev).toBeVisible()
    await expect(page.locator('td[data-date="2026-10-09"] [data-ev]')).toContainText('Review design')
    await ev.dragTo(page.locator('td[data-date="2026-10-12"]'))
    await expect.poll(() => backend.read('pages/work/projects/alpha.md')).toContain('deadline:: 2026-10-12')
  })

  test('view switcher, show query and save to sidebar', async ({ backend, page }) => {
    backend.write('pages/dash.md', PAGE('task: todo\nns: work'))
    await backend.open('/page/dash')
    await page.getByRole('button', { name: 'table', exact: true }).click()
    await expect(page.getByTestId('query-table')).toBeVisible()
    await page.getByRole('button', { name: 'show query' }).click()
    await expect(page.locator('pre.code')).toContainText('ns: work')
    await page.getByRole('button', { name: 'save', exact: true }).click()
    await page.getByRole('textbox', { name: 'Name' }).fill('Open work')
    await page.getByRole('button', { name: 'Save to sidebar' }).click()
    await expect(page.getByRole('link', { name: 'Open work' })).toBeVisible()
    expect(backend.read('.notatnik/config.toml')).toContain('Open work')
    await page.getByRole('link', { name: 'Open work' }).click()
    await expect(page.getByTestId('query-count')).toHaveText('5 results')
  })

  test('list view: editing a block inside results writes to the source page', async ({ backend, page }) => {
    backend.write('pages/dash.md', PAGE('page: work/tasks\ntask: todo'))
    await backend.open('/page/dash')
    await page.locator('[data-testid="query-list"]').getByText('Work root task').click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    await page.keyboard.press('End')
    await page.keyboard.type(' edited')
    await page.keyboard.press('Escape')
    await expect.poll(() => backend.read('pages/work/tasks.md')).toContain('- TODO Work root task edited')
  })
})

test.describe('autocomplete', () => {
  test.use({ fixture: 'queries' })

  test('clause keys inside a query block', async ({ backend, page }) => {
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
    await page.keyboard.type('ta')
    const tip = page.locator('.cm-tooltip-autocomplete')
    await expect(tip).toContainText('task', { timeout: 5000 })
    await expect(tip).toContainText('tag')
  })

  // the editor autosaves after 500 ms without typing: one clause per test keeps each below that
  const openQueryEnd = async (
    backend: { write: (p: string, t: string) => void; open: (p: string) => Promise<void> },
    page: import('@playwright/test').Page,
  ) => {
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
  }

  test('values: `task: ` offers the states', async ({ backend, page }) => {
    await openQueryEnd(backend, page)
    await page.keyboard.type('task: do')
    const tip = page.locator('.cm-tooltip-autocomplete')
    await expect(tip).toContainText('doing', { timeout: 5000 })
    await expect(tip).toContainText('done')
  })

  test('values: a key picked from the list goes straight on to its values', async ({ backend, page }) => {
    await openQueryEnd(backend, page)
    await page.keyboard.type('childr')
    const tip = page.locator('.cm-tooltip-autocomplete')
    await expect(tip).toContainText('children', { timeout: 5000 })
    await page.waitForTimeout(150)
    await page.keyboard.press('Enter')
    await expect(tip).toContainText('show the sub-blocks of each result', { timeout: 5000 })
  })

  test('values: a sort field goes on to asc / desc', async ({ backend, page }) => {
    await openQueryEnd(backend, page)
    await page.keyboard.type('sort: dead')
    const tip = page.locator('.cm-tooltip-autocomplete')
    await expect(tip).toContainText('deadline', { timeout: 5000 })
    await page.waitForTimeout(150)
    await page.keyboard.press('Enter')
    await expect(tip).toContainText('latest first', { timeout: 5000 })
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('block-editor')).toContainText('sort: deadline ')
  })

  test('property keys and values after `::`', async ({ backend, page }) => {
    backend.write('pages/p.md', '- A block\n  rating:: 3\n')
    await backend.open('/page/p')
    await page.getByText('A block').click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    await page.keyboard.press('Control+End')
    await page.keyboard.press('Shift+Enter')
    await page.keyboard.type('sta')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('status', { timeout: 5000 })
    await expect(page.locator('.cm-tooltip-autocomplete [aria-selected="true"]')).toBeVisible()
    await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
    await page.keyboard.press('Enter')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('draft', { timeout: 5000 })
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('reading')
  })
})

test.describe('properties clause', () => {
  test.use({ fixture: 'queries' })

  test('properties: none hides the property lines of the results, a list keeps only those', async ({
    backend,
    page,
  }) => {
    const q = (extra: string) =>
      PAGE(`page: work/projects/beta\ntext: Beta notes${extra ? `\n${extra}` : ''}`)
    const result = page.getByTestId('query-block').first().getByTestId('query-list')
    backend.write('pages/pr.md', q(''))
    await backend.open('/page/pr')
    await expect(result).toContainText('Beta notes')
    await expect(result).toContainText('rating')
    await expect(result).toContainText('status')
    backend.write('pages/pr.md', q('properties: status'))
    await backend.open('/page/pr')
    await expect(result).toContainText('Beta notes')
    await expect(result).toContainText('status')
    await expect(result).not.toContainText('rating')
    backend.write('pages/pr.md', q('properties: none'))
    await backend.open('/page/pr')
    await expect(result).toContainText('Beta notes')
    await expect(result).not.toContainText('status')
    await expect(result).not.toContainText('rating')
  })
})

test.describe('example queries', () => {
  test.use({ fixture: 'queries' })

  test('in Settings the examples are folded until asked for', async ({ backend, page }) => {
    await backend.open('/settings')
    await page.getByRole('tab', { name: /Templates/ }).click()
    const list = page.getByTestId('example-queries')
    await expect(page.getByRole('heading', { name: 'Example queries' })).toBeVisible()
    await expect(list).toBeHidden()
    await page.getByText(/Show the \d+ examples/).click()
    await expect(list).toBeVisible()
    await expect(list).toContainText('Pages of one series')
    await page.getByText(/Hide the \d+ examples/).click()
    await expect(list).toBeHidden()
  })

  test('/query lists examples; the picked one becomes a query block in an empty block', async ({
    backend,
    page,
  }) => {
    backend.write('pages/ex.md', '- \n')
    await backend.open('/page/ex')
    await page
      .getByRole('tree', { name: 'Blocks' })
      .getByRole('button', { name: 'Zoom into block' })
      .first()
      .hover()
    await page.locator('.body').first().click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    await page.keyboard.type('/query')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Query example…', { timeout: 5000 })
    await page.waitForTimeout(150)
    await page.keyboard.press('Enter')
    const picker = page.getByTestId('query-example-picker')
    await expect(picker).toBeVisible()
    await expect(picker).toContainText('Pages of one series')
    await picker.getByLabel('Filter examples').fill('without a date')
    await expect(picker.getByRole('option')).toHaveCount(1)
    await page.keyboard.press('Enter')
    await expect(picker).toBeHidden()
    await expect(page.getByTestId('block-editor')).toContainText('```query')
    await expect(page.getByTestId('block-editor')).toContainText('scheduled: none')
    await page.keyboard.press('Escape')
    await expect
      .poll(() => backend.read('pages/ex.md'))
      .toContain('- ```query\n  task: todo\n  scheduled: none')
    await expect(page.getByTestId('query-block')).toBeVisible()
  })

  test('the picker links to the list of all examples in Settings', async ({ backend, page }) => {
    backend.write('pages/ex.md', '- \n')
    await backend.open('/page/ex')
    await page.locator('.body').first().click()
    await page.keyboard.type('/query')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Query example…', { timeout: 5000 })
    await page.waitForTimeout(150)
    await page.keyboard.press('Enter')
    await page.getByTestId('browse-examples').click()
    await expect(page).toHaveURL(/#\/settings/)
    // the link opens the folded list
    const list = page.getByTestId('example-queries')
    await expect(list).toBeVisible()
    await expect(list).toContainText('Pages of one series')
    await expect(page.getByRole('heading', { name: 'Example queries' })).toBeInViewport()
  })
})

test('a query block opens for editing from its header, its frame and its shown query text', async ({
  backend,
  page,
}) => {
  backend.write('pages/q.md', '- ```query\n  task: todo\n  ```\n- TODO find me\n')
  await backend.open('/page/q')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  const q = blocks.getByTestId('query-block').first()
  await expect(q).toContainText('find me')
  // the header (a wide target, with a hint) edits the block
  await q.getByText('click here to edit the query').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await expect(page.getByTestId('block-editor')).toContainText('task: todo')
  await page.keyboard.press('Escape')
  // so does the frame around it and the text of "show query"
  await q.getByRole('button', { name: 'show query' }).click()
  await q.locator('pre.code').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  // a click on a result is still the result's own (it does not open the query for editing)
  await blocks.getByTestId('query-block').first().getByText('find me').first().click()
  await expect(page.getByTestId('block-editor')).toBeFocused() // that editor is the task block's own
  await expect(page.getByTestId('block-editor')).not.toContainText('task: todo')
})
