import { expect, test } from './fixtures'

test('Ctrl+Enter cycles the task state; only the first line and done:: change on disk', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- plain task text\n- other\n')
  await backend.open('/page/t')
  await page.getByText('plain task text').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  const editor = page.getByTestId('block-editor')
  await page.keyboard.press('Control+Enter')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- TODO plain task text\n- other\n')
  await expect(editor).toContainText('TODO plain task text')
  await page.keyboard.press('Control+Enter')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- DOING plain task text\n- other\n')
  await expect(editor).toContainText('DOING plain task text')
  await page.keyboard.press('Control+Enter')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toMatch(/^- DONE plain task text\n {2}done:: 2026-10-03T\d\d:\d\d\n- other\n$/)
  await expect(editor).toContainText('done::')
  await page.keyboard.press('Control+Enter')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- plain task text\n- other\n')
})

test('clicking the marker cycles; DONE is struck through', async ({ backend, page }) => {
  backend.write('pages/t.md', '- TODO a task\n')
  await backend.open('/page/t')
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="TODO"]').click()
  await expect(page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="DOING"]')).toBeVisible()
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="DOING"]').click()
  await expect(page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="DONE"]')).toBeVisible()
  await expect(page.getByRole('tree', { name: 'Blocks' }).locator('.content.done')).toBeVisible()
  expect(backend.read('pages/t.md')).toContain('done::')
})

test('completing a weekly repeating task resets it and logs the date', async ({ backend, page }) => {
  backend.write(
    'pages/r.md',
    '- TODO Water plants\n  scheduled:: 2026-10-03\n  repeat:: weekly\n  completed:: 2026-09-26\n- after\n',
  )
  await backend.open('/page/r')
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="TODO"]').click() // → DOING
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="DOING"]').click() // → DONE → reset
  await expect
    .poll(() => backend.read('pages/r.md'))
    .toBe(
      '- TODO Water plants\n  scheduled:: 2026-10-10\n  repeat:: weekly\n  completed:: 2026-09-26, 2026-10-03\n- after\n',
    )
  await expect(page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="TODO"]')).toBeVisible()
  // completed:: is hidden in the rendered view
  await expect(page.getByText('completed:')).toHaveCount(0)
})

test('deadline chip: set a date, clear it; overdue chips are styled', async ({ backend, page }) => {
  backend.write(
    'pages/c.md',
    '- TODO with deadline\n  deadline:: 2026-10-01\n- TODO future\n  deadline:: 2026-10-20\n',
  )
  await backend.open('/page/c')
  await expect(
    page.getByRole('tree', { name: 'Blocks' }).locator('[data-chip="deadline"]').first(),
  ).toHaveClass(/overdue/)
  await expect(
    page.getByRole('tree', { name: 'Blocks' }).locator('[data-chip="deadline"]').nth(1),
  ).not.toHaveClass(/overdue/)
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-chip="deadline"]').nth(1).click()
  await page.getByTestId('date-picker').getByRole('textbox').fill('2026-10-10')
  await page.keyboard.press('Enter')
  await expect.poll(() => backend.read('pages/c.md')).toContain('  deadline:: 2026-10-10\n')
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-chip="deadline"]').nth(1).click()
  await page.getByTestId('date-picker').getByRole('button', { name: 'Clear' }).click()
  await expect
    .poll(() => backend.read('pages/c.md'))
    .toBe('- TODO with deadline\n  deadline:: 2026-10-01\n- TODO future\n')
})

test('subtask progress counts all descendants (task parents only) and updates immediately', async ({
  backend,
  page,
}) => {
  backend.write('pages/p.md', '- WAITING Project\n  - DONE one\n  - TODO two\n    - TODO nested\n')
  await backend.open('/page/p')
  await expect(page.getByTestId('progress').first()).toHaveText('1/3')
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="TODO"]').first().click()
  await page.getByRole('tree', { name: 'Blocks' }).locator('[data-task="DOING"]').first().click()
  await expect(page.getByTestId('progress').first()).toHaveText('2/3')
})

test.describe('tasks fixture', () => {
  test.use({ fixture: 'tasks' })
  test('the agenda can be shown as list, table, kanban and timeline, with the same filters', async ({
    backend,
    page,
  }) => {
    await backend.open('/agenda')
    const modes = page.getByRole('group', { name: 'Agenda view' })
    await modes.getByRole('button', { name: 'table' }).click()
    const results = page.getByTestId('query-block')
    // every open task (overdue, today, upcoming, later and undated), through the table view
    await expect(page.getByTestId('query-count')).toHaveText(/^\d+ results?$/)
    const total = Number((await page.getByTestId('query-count').textContent())!.split(' ')[0])
    expect(total).toBeGreaterThan(20)
    await expect(results.locator('table')).toBeVisible()
    // the filters apply to every view
    await page.getByRole('search', { name: 'Agenda filters' }).getByLabel('Namespace').fill('work')
    await expect(async () => {
      const n = Number((await page.getByTestId('query-count').textContent())!.split(' ')[0])
      expect(n).toBeLessThan(total)
    }).toPass()
    await modes.getByRole('button', { name: 'kanban' }).click()
    await expect(results).toContainText('doing')
    await modes.getByRole('button', { name: 'timeline' }).click()
    await modes.getByRole('button', { name: 'list' }).click()
    await expect(results.locator('[data-testid="query-count"]')).toBeVisible()
    // the mode is remembered on this device
    await page.reload()
    await expect(modes.getByRole('button', { name: 'list' })).toHaveAttribute('aria-pressed', 'true')
    await modes.getByRole('button', { name: 'sections' }).click()
    await expect(page.locator('[data-section="overdue"]')).toBeVisible()
  })
  test('sections honour group and sort', async ({ backend, page }) => {
    await backend.open('/agenda')
    const overdue = page.locator('[data-section="overdue"]')
    await expect(overdue.locator('[data-testid="agenda-item"]')).toHaveCount(7)
    const sources = async () =>
      overdue
        .getByTestId('agenda-source')
        .allTextContents()
        .then((l) => l.map((t) => t.replace(/^↗ /, '').split(' › ')[0]))
    // sorted by page, the page names are in order
    await page.getByTestId('agenda-sort').selectOption('page asc')
    await expect(async () => {
      const s = await sources()
      expect(s).toEqual([...s].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase())))
    }).toPass()
    // grouped by page: a heading per page, the counts add up, and the items stay in the section
    await page.getByTestId('agenda-group').selectOption('page')
    const heads = overdue.getByTestId('agenda-group-head')
    await expect(heads.first()).toBeVisible()
    await expect(overdue.locator('[data-testid="agenda-item"]')).toHaveCount(7)
    const counts = (await heads.allTextContents()).map((t) => Number(t.trim().split(' ').pop()))
    expect(counts.reduce((a, b) => a + b, 0)).toBe(7)
    // and back to the plain agenda
    await page.getByTestId('agenda-group').selectOption('')
    await expect(heads).toHaveCount(0)
  })
  test('agenda sections match the expectation and completing removes the task', async ({ backend, page }) => {
    await backend.open('/agenda')
    await expect(page.locator('[data-section="overdue"] [data-testid="agenda-item"]')).toHaveCount(7)
    await expect(page.locator('[data-section="today"] [data-testid="agenda-item"]')).toHaveCount(1)
    await expect(page.locator('[data-section="upcoming"] h3')).toHaveCount(4)
    await expect(page.locator('[data-section="later"] [data-testid="agenda-item"]')).toHaveCount(1)
    await expect(page.locator('[data-section="undated"] [data-testid="agenda-item"]')).toHaveCount(8)
    // complete today's task from the agenda
    await page.locator('[data-section="today"] [data-task="TODO"]').click()
    await page.locator('[data-section="today"] [data-task="DOING"]').click()
    await expect(page.locator('[data-section="today"] [data-testid="agenda-item"]')).toHaveCount(0)
    expect(backend.read('pages/dates.md')).toContain('- DONE Due today\n  deadline:: 2026-10-03\n  done::')
  })
  test('a task in the agenda links to its note and the task is flashed there', async ({ backend, page }) => {
    await backend.open('/agenda')
    const item = page.locator('[data-section="today"] [data-testid="agenda-item"]')
    await expect(item).toHaveCount(1)
    await expect(item.getByTestId('agenda-source')).toContainText('dates')
    await item.getByTestId('agenda-source').click()
    await expect(page).toHaveURL(/#\/page\/dates$/)
    // the whole note is shown (not just the task) and the task is highlighted for a moment
    await expect(page.getByText('Due today')).toBeVisible()
    await expect(page.locator('.block.flash')).toBeVisible()
    await expect(page.locator('.block.flash')).toContainText('Due today')
  })
  test("the source of a task sits in the task's own row, to its right", async ({ backend, page }) => {
    await backend.open('/agenda')
    const item = page.locator('[data-section="today"] [data-testid="agenda-item"]')
    await expect(item).toHaveCount(1)
    const task = await item.locator('.task').boundingBox()
    const source = await item.getByTestId('agenda-source').boundingBox()
    expect(task && source).toBeTruthy()
    // right of the task text, and starting at the task's first row (not above it)
    expect(source!.x).toBeGreaterThan(task!.x + task!.width - 1)
    expect(source!.y).toBeGreaterThanOrEqual(task!.y - 1)
    expect(source!.y).toBeLessThan(task!.y + 40)
    await expect(item.getByTestId('agenda-source')).toHaveText('dates')
  })
  test('the filter fields suggest pages and a pick is written as the plain name', async ({
    backend,
    page,
  }) => {
    backend.write('pages/zeta-project.md', '- TODO zeta task\n')
    await backend.open('/agenda')
    const filters = page.getByRole('search', { name: 'Agenda filters' })
    await filters.getByRole('combobox', { name: 'Tag' }).fill('zeta')
    await page.getByRole('option', { name: /zeta-project/ }).click()
    await expect(filters.getByRole('combobox', { name: 'Tag' })).toHaveValue('zeta-project')
    await filters.getByRole('combobox', { name: 'Namespace' }).fill('zeta')
    await page.getByRole('option', { name: /zeta-project/ }).click()
    await expect(filters.getByRole('combobox', { name: 'Namespace' })).toHaveValue('zeta-project')
  })
  test('namespace filter limits the agenda', async ({ backend, page }) => {
    await backend.open('/agenda')
    await page.getByRole('combobox', { name: 'Namespace' }).fill('work')
    await expect(page.locator('[data-section="overdue"] [data-testid="agenda-item"]')).toHaveCount(0)
    await expect(page.locator('[data-section="upcoming"] [data-testid="agenda-item"]')).toHaveCount(1)
  })
})

test("a task scheduled today is not added to today's journal", async ({ backend, page }) => {
  backend.write('pages/anywhere.md', '- TODO call mum\n  scheduled:: 2026-10-03\n')
  await backend.open('/journals')
  // the journal shows only its own blocks: due tasks live in the agenda
  await expect(page.getByTestId('journals')).toBeVisible()
  await expect(page.getByTestId('journals')).not.toContainText('call mum')
  expect(backend.read('journals/2026/2026-10-03.md')).not.toContain('call mum')
})

test('agenda board and timeline: cards and tasks can be dragged', async ({ backend, page }) => {
  backend.write('pages/b.md', '- TODO alpha task\n  deadline:: 2026-10-08\n- TODO beta task\n')
  await backend.open('/agenda')
  const modes = page.getByRole('group', { name: 'Agenda view' })
  await modes.getByRole('button', { name: 'kanban' }).click()
  const board = page.getByTestId('query-kanban')
  await board
    .locator('[data-card]')
    .filter({ hasText: 'beta task' })
    .dragTo(board.locator('[data-col="doing"]'))
  await expect.poll(() => backend.read('pages/b.md')).toContain('- DOING beta task')
  await modes.getByRole('button', { name: 'timeline' }).click()
  const ev = page.locator('[data-ev]', { hasText: 'alpha task' })
  await expect(ev).toBeVisible()
  await ev.dragTo(page.locator('td[data-date="2026-10-12"]'))
  await expect.poll(() => backend.read('pages/b.md')).toContain('deadline:: 2026-10-12')
  // a board grouped by page has nothing to change by moving: cards are not draggable
  await modes.getByRole('button', { name: 'kanban' }).click()
  await page.getByTestId('agenda-group').selectOption('page')
  await expect(board.locator('[data-card]').first()).toHaveAttribute('data-movable', 'false')
})

test('kanban cards: arrows move them, a click opens the note with the task', async ({ backend, page }) => {
  backend.write('pages/b.md', '- TODO alpha task\n- TODO beta task\n')
  await backend.open('/agenda')
  await page.getByRole('group', { name: 'Agenda view' }).getByRole('button', { name: 'kanban' }).click()
  const board = page.getByTestId('query-kanban')
  const card = board.locator('[data-card]').filter({ hasText: 'alpha task' })
  await card.hover()
  await card.getByRole('button', { name: 'Move to doing' }).click()
  await expect.poll(() => backend.read('pages/b.md')).toContain('- DOING alpha task')
  // a plain click lands on the source page
  await board.locator('[data-card]').filter({ hasText: 'beta task' }).click()
  await expect(page).toHaveURL(/#\/page\/b/)
})

test('agenda kanban keeps done cards on the board', async ({ backend, page }) => {
  backend.write('pages/b.md', '- TODO alpha task\n- DONE finished earlier\n')
  await backend.open('/agenda')
  await page.getByRole('group', { name: 'Agenda view' }).getByRole('button', { name: 'kanban' }).click()
  const board = page.getByTestId('query-kanban')
  // finished tasks are on the board from the start
  await expect(board.locator('[data-col="done"]')).toContainText('finished earlier')
  // and a card moved to done stays there
  const card = board.locator('[data-card]').filter({ hasText: 'alpha task' })
  await card.hover()
  await card.getByRole('button', { name: 'Move to doing' }).click()
  for (const to of ['waiting', 'done']) {
    const c = board.locator('[data-card]').filter({ hasText: 'alpha task' })
    await c.hover()
    await c.getByRole('button', { name: `Move to ${to}` }).click()
  }
  await expect(board.locator('[data-col="done"]')).toContainText('alpha task')
})

test('priority shows after the task marker with the configured labels', async ({ backend, page }) => {
  backend.write(
    'pages/t.md',
    '- TODO urgent thing\n  priority:: high\n- TODO middle thing\n  priority:: B\n- TODO plain thing\n',
  )
  await backend.open('/page/t')
  const row = (t: string) =>
    page.getByRole('tree', { name: 'Blocks' }).locator('.content').filter({ hasText: t })
  await expect(row('urgent thing').locator('[data-chip="priority"]')).toHaveText('[#A]')
  await expect(row('middle thing').locator('[data-chip="priority"]')).toHaveText('[#B]')
  // no priority by default
  await expect(row('plain thing').locator('[data-chip="priority"]')).toHaveCount(0)
  // the label is after the marker, before the text
  const order = await row('urgent thing').evaluate((el) =>
    [...el.querySelectorAll('.marker, .prio, .nodes')].map((n) => n.className.split(' ')[0]),
  )
  expect(order).toEqual(['marker', 'prio', 'nodes'])
  // the labels are a graph setting
  await page.getByRole('link', { name: 'Settings' }).click()
  await page.getByRole('tab', { name: 'Tasks' }).click()
  await page.getByLabel('Label of priority 1').fill('P1')
  await page.getByLabel('Label of priority 2').fill('P2')
  await page.getByLabel('Label of priority 3').fill('P3')
  // and there can be any number of them
  await page.getByRole('button', { name: '+ Add' }).click()
  await page.getByLabel('Label of priority 4').fill('P4')
  await page.keyboard.press('Tab') // commits the field
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('"P4"')
  await page.evaluate(() => (location.hash = '#/page/t'))
  await expect(row('urgent thing').locator('[data-chip="priority"]')).toHaveText('[#P1]')
  // `priority:: B` is not a label any more: a warning, no priority
  await expect(row('middle thing').locator('[data-chip="priority"]')).toHaveCount(0)
  // / commands: one per label, and one to clear
  await row('plain thing').click()
  await page.keyboard.press('End')
  await page.keyboard.type(' /priority-p4')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Priority P4/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect.poll(() => backend.read('pages/t.md')).toContain('- TODO plain thing\n  priority:: P4')
  await page.keyboard.type(' /priority-none')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/No priority/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect.poll(() => backend.read('pages/t.md')).not.toContain('priority:: P4')
})

test('a priority added to config.toml by hand is offered by / without restarting', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- TODO some thing\n')
  await backend.open('/page/t')
  await expect(page.getByRole('tree', { name: 'Blocks' })).toContainText('some thing')
  const cfg = backend.read('.notatnik/config.toml')
  backend.write(
    '.notatnik/config.toml',
    cfg.replace('[tasks]\n', '[tasks]\npriorities = ["A", "B", "C", "D"]\n'),
  )
  await page.getByRole('tree', { name: 'Blocks' }).getByText('some thing').click()
  await page.keyboard.press('End')
  // the labels reach the browser a moment after the file was saved
  await expect(async () => {
    await page.keyboard.type(' /priority-d')
    await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Priority D/, { timeout: 500 })
  }).toPass({ timeout: 8000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect.poll(() => backend.read('pages/t.md')).toContain('priority:: D')
})
