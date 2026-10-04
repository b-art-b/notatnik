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
