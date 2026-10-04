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
  await page.locator('[data-task="TODO"]').click()
  await expect(page.locator('[data-task="DOING"]')).toBeVisible()
  await page.locator('[data-task="DOING"]').click()
  await expect(page.locator('[data-task="DONE"]')).toBeVisible()
  await expect(page.locator('.content.done')).toBeVisible()
  expect(backend.read('pages/t.md')).toContain('done::')
})

test('completing a weekly repeating task resets it and logs the date', async ({ backend, page }) => {
  backend.write(
    'pages/r.md',
    '- TODO Water plants\n  scheduled:: 2026-10-03\n  repeat:: weekly\n  completed:: 2026-09-26\n- after\n',
  )
  await backend.open('/page/r')
  await page.locator('[data-task="TODO"]').click() // → DOING
  await page.locator('[data-task="DOING"]').click() // → DONE → reset
  await expect
    .poll(() => backend.read('pages/r.md'))
    .toBe(
      '- TODO Water plants\n  scheduled:: 2026-10-10\n  repeat:: weekly\n  completed:: 2026-09-26, 2026-10-03\n- after\n',
    )
  await expect(page.locator('[data-task="TODO"]')).toBeVisible()
  // completed:: is hidden in the rendered view
  await expect(page.getByText('completed:')).toHaveCount(0)
})

test('deadline chip: set a date, clear it; overdue chips are styled', async ({ backend, page }) => {
  backend.write(
    'pages/c.md',
    '- TODO with deadline\n  deadline:: 2026-10-01\n- TODO future\n  deadline:: 2026-10-20\n',
  )
  await backend.open('/page/c')
  await expect(page.locator('[data-chip="deadline"]').first()).toHaveClass(/overdue/)
  await expect(page.locator('[data-chip="deadline"]').nth(1)).not.toHaveClass(/overdue/)
  await page.locator('[data-chip="deadline"]').nth(1).click()
  await page.getByLabel('deadline').fill('2026-10-10')
  await expect.poll(() => backend.read('pages/c.md')).toContain('  deadline:: 2026-10-10\n')
  await page.locator('[data-chip="deadline"]').nth(1).click()
  await page.getByRole('button', { name: 'clear' }).click()
  await expect
    .poll(() => backend.read('pages/c.md'))
    .toBe('- TODO with deadline\n  deadline:: 2026-10-01\n- TODO future\n')
})

test('subtask progress counts all descendants and updates immediately', async ({ backend, page }) => {
  backend.write('pages/p.md', '- Project\n  - DONE one\n  - TODO two\n    - TODO nested\n')
  await backend.open('/page/p')
  await expect(page.getByTestId('progress').first()).toHaveText('1/3')
  await page.locator('[data-task="TODO"]').first().click()
  await page.locator('[data-task="DOING"]').first().click()
  await expect(page.getByTestId('progress').first()).toHaveText('2/3')
})

test.describe('tasks fixture', () => {
  test.use({ fixture: 'tasks' })
  test('agenda sections match the expectation and completing removes the task', async ({ backend, page }) => {
    await backend.open('/agenda')
    await expect(page.locator('[data-section="overdue"] [data-testid="agenda-item"]')).toHaveCount(7)
    await expect(page.locator('[data-section="today"] [data-testid="agenda-item"]')).toHaveCount(1)
    await expect(page.locator('[data-section="upcoming"] h3')).toHaveCount(4)
    await expect(page.locator('[data-section="undated"] [data-testid="agenda-item"]')).toHaveCount(8)
    // complete today's task from the agenda
    await page.locator('[data-section="today"] [data-task="TODO"]').click()
    await page.locator('[data-section="today"] [data-task="DOING"]').click()
    await expect(page.locator('[data-section="today"] [data-testid="agenda-item"]')).toHaveCount(0)
    expect(backend.read('pages/dates.md')).toContain('- DONE Due today\n  deadline:: 2026-10-03\n  done::')
  })
  test('namespace filter limits the agenda', async ({ backend, page }) => {
    await backend.open('/agenda')
    await page.getByRole('textbox', { name: 'Namespace' }).fill('work')
    await expect(page.locator('[data-section="overdue"] [data-testid="agenda-item"]')).toHaveCount(0)
    await expect(page.locator('[data-section="upcoming"] [data-testid="agenda-item"]')).toHaveCount(1)
  })
})

test("a task scheduled today shows in today's journal without being written into it", async ({
  backend,
  page,
}) => {
  backend.write('pages/anywhere.md', '- TODO call mum\n  scheduled:: 2026-10-03\n')
  await backend.open('/journals')
  await expect(page.getByTestId('today-tasks')).toContainText('call mum')
  expect(backend.read('journals/2026/2026-10-03.md')).not.toContain('call mum')
})
