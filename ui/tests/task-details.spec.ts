import { expect, test } from './fixtures'

// TK-13: "Task details: compact" shows a task's date, repeat and logbook as small indicators; the details open
// on hover, focus or click. The agenda always shows everything.

const PAGE =
  '- TODO pay rent\n  deadline:: 2026-10-01\n  repeat:: +1m\n  :LOGBOOK:\n  2026-10-02 10:00 called the landlord\n  :END:\n- TODO later\n  scheduled:: 2026-12-24\n'

test('compact task details: indicators in the row, details on hover, agenda unchanged', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', PAGE)
  await backend.open('/settings')
  await page.getByRole('radiogroup', { name: 'Task details' }).getByRole('radio', { name: 'Compact' }).check()
  await backend.open('/page/t')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  // the lines under the task are gone; one indicator per kind stays in its row
  await expect(blocks.locator('.chips')).toHaveCount(0)
  await expect(blocks.getByTestId('logbook')).toHaveCount(0)
  const meta = blocks.getByTestId('task-meta').first()
  const date = meta.locator('[data-indicator="date"]')
  await expect(date).toContainText('2d overdue')
  // overdue keeps its colour: urgency is visible without opening anything
  const color = await date.evaluate((e) => getComputedStyle(e).color)
  const plain = await meta.locator('[data-indicator="logbook"]').evaluate((e) => getComputedStyle(e).color)
  expect(color).not.toBe(plain)
  await expect(meta.locator('[data-indicator="repeat"]')).toBeVisible()
  await expect(meta.locator('[data-indicator="logbook"]')).toContainText('1')
  // a later schedule shows as a date, with the scheduled glyph
  await expect(blocks.getByTestId('task-meta').nth(1).locator('[data-indicator="date"]')).toContainText(
    /2026|Dec/,
  )
  // the indicator sits in the task's own row, to the right of its text
  const text = await blocks.getByText('pay rent').first().boundingBox()
  const box = await date.boundingBox()
  expect(box!.x).toBeGreaterThan(text!.x + text!.width - 1)
  expect(Math.abs(box!.y - text!.y)).toBeLessThan(20)
  // hover opens the details with the usual chips and logbook; leaving closes them
  await expect(meta.getByTestId('task-details')).toHaveCount(0)
  await date.hover()
  const pop = meta.getByTestId('task-details')
  await expect(pop).toBeVisible()
  await expect(pop.locator('[data-chip="deadline"]')).toContainText('2026-10-01')
  await expect(pop.getByTestId('logbook')).toContainText('called the landlord')
  // an open logbook must not squeeze the popover into a narrow column
  await pop.getByRole('button', { name: 'Logbook:' }).click()
  const width = (await pop.boundingBox())!.width
  expect(width).toBeGreaterThan(220)
  await page.mouse.move(5, 5)
  await expect(pop).toHaveCount(0)
  // a click keeps it open until Escape
  await date.click()
  await page.mouse.move(5, 5)
  await expect(pop).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(pop).toHaveCount(0)
  // the agenda shows the full lines whatever the setting
  await backend.open('/agenda')
  await expect(page.locator('[data-chip="deadline"]').first()).toBeVisible()
})

test('the default setting shows everything as before', async ({ backend, page }) => {
  backend.write('pages/t.md', PAGE)
  await backend.open('/page/t')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await expect(blocks.locator('[data-chip="deadline"]').first()).toBeVisible()
  await expect(blocks.getByTestId('task-meta')).toHaveCount(0)
})

// PR-8: the same for block properties, as its own setting
test('compact block properties: one indicator, the card in a popover, edits work there', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/t.md',
    '- first\n  type:: partner\n  client:: [[Acme]]\n- TODO a task\n  deadline:: 2026-12-24\n  owner:: me\n',
  )
  await backend.open('/settings')
  await page
    .getByRole('radiogroup', { name: 'Block properties' })
    .getByRole('radio', { name: 'Compact' })
    .check()
  await backend.open('/page/t')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  // no card under the blocks; one indicator per block with properties, with the count
  await expect(blocks.getByTestId('props')).toHaveCount(0)
  const first = blocks.getByTestId('task-meta').first()
  await expect(first.locator('[data-indicator="props"]')).toContainText('2')
  await expect(first.locator('[data-indicator="props"]')).toHaveAttribute('title', /type: partner/)
  // the task's own date chip stays (task details are still "always shown"); only `owner` counts as a property
  const task = blocks.getByTestId('task-meta').nth(1)
  await expect(task.locator('[data-indicator="props"]')).toContainText('1')
  await expect(blocks.locator('[data-chip="deadline"]')).toBeVisible()
  // hover opens the card; editing there changes only that line
  await first.locator('[data-indicator="props"]').hover()
  const card = first.getByTestId('props')
  await expect(card).toContainText('partner')
  await card.locator('.prop[data-key="type"]').hover()
  await card.getByRole('button', { name: 'Edit type' }).click()
  await card.getByRole('combobox', { name: 'Value of type' }).fill('customer')
  await page.keyboard.press('Enter')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toContain('- first\n  type:: customer\n  client:: [[Acme]]\n')
  // the agenda and the default setting are unchanged: see the other tests
})
