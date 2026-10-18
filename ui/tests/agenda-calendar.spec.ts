import { expect, test, type Page } from './fixtures'

// The calendar mode of the Agenda (CV-1..CV-8) against the dev server's fake calendar server. "Today" is
// Saturday 2026-10-03, so the week shown is Monday 28 Sep to Sunday 4 Oct.
test.use({ fakeCalendar: true, viewport: { width: 1400, height: 1500 } })

const PAGE = [
  '- TODO Range task',
  '  scheduled:: 2026-10-01 09:00-10:30',
  '- TODO Short task',
  '  scheduled:: 2026-10-01 09:00',
  '- TODO Day task',
  '  deadline:: 2026-10-02',
  '- TODO Due task',
  '  deadline:: 2026-10-02 17:00',
  '- TODO Undated task',
  '',
].join('\n')

async function setUp(backend: Parameters<Parameters<typeof test>[2]>[0]['backend'], page: Page) {
  backend.write('pages/Plan.md', PAGE)
  await backend.open('/')
  const list = await backend.call<{ calendars: { href: string; name: string }[] }>('calendar_connect', {
    server: 'http://localhost:8080',
    user: 'me',
    password: 'secret',
  })
  const birthdays = list.calendars.find((c) => c.name === 'Birthdays')!.href
  await backend.call('calendar_save', {
    enabled: true,
    read: [birthdays],
    tasks: null,
    import_tasks: false,
    undated_tasks: false,
  })
  await backend.call('calendar_sync')
  await page.reload()
  await backend.open('/agenda')
  await page.getByRole('button', { name: 'calendar', exact: true }).click()
  await expect(page.getByTestId('cal-grid')).toBeVisible()
  // everything is loaded (the all-day strip grows with its items, which moves the hours)
  await expect(page.getByTestId('cal-item').filter({ hasText: 'Day task' })).toHaveCount(1)
  await expect(page.getByTestId('cal-item').filter({ hasText: 'Team standup' })).toHaveCount(7)
  // the hours from midnight are in view for the pointer maths of the tests
  await page.getByTestId('cal-scroll').evaluate((el) => (el.scrollTop = 0))
  return birthdays
}

const item = (page: Page, title: string) => page.getByTestId('cal-item').filter({ hasText: title }).first()

test('a week with hours: lengths, all-day strip, overlaps, events and the tray', async ({
  backend,
  page,
}) => {
  await setUp(backend, page)
  await expect(page.getByTestId('cal-heading')).toHaveText('28 Sep – 4 Oct 2026')

  // a scheduled start with an end is a block of that length; without an end it takes an hour
  const range = item(page, 'Range task')
  await expect(range).toHaveAttribute('data-start', '540')
  await expect(range).toHaveAttribute('data-end', '630')
  await expect(item(page, 'Short task')).toHaveAttribute('data-end', '600')
  // a timed deadline is the hour before it; a plain date goes to the strip
  await expect(item(page, 'Due task')).toHaveAttribute('data-start', '960')
  await expect(item(page, 'Due task')).toHaveAttribute('data-end', '1020')
  await expect(
    page.getByTestId('cal-allday').locator('[data-testid="cal-item"]', { hasText: 'Day task' }),
  ).toHaveCount(1)

  // the daily event is on every day of the week, at its time
  const standups = page.getByTestId('cal-item').filter({ hasText: 'Team standup' })
  await expect(standups).toHaveCount(7)
  await expect(standups.first()).toHaveAttribute('data-kind', 'event')
  await expect(standups.first()).toHaveAttribute('data-start', '540')
  await expect(standups.first()).toHaveAttribute('data-end', '555')

  // blocks that overlap in time (Thursday 09:00: range, short, standup) share the width
  const thursday = page.locator('[data-col="2026-10-01"]')
  const colBox = (await thursday.boundingBox())!
  const lefts = new Set<number>()
  for (const t of ['Range task', 'Short task', 'Team standup']) {
    const box = (await thursday.getByTestId('cal-item').filter({ hasText: t }).boundingBox())!
    lefts.add(Math.round(box.x))
    expect(box.width).toBeLessThan(colBox.width * 0.6)
  }
  expect(lefts.size).toBe(3)

  // the task without a date waits in the tray
  await expect(page.getByTestId('cal-tray-item')).toHaveText(['Undated task'])
  // today's column has the line of the current time
  await expect(page.locator('[data-col="2026-10-03"]').getByTestId('cal-now')).toBeVisible()
})

test('day, week and month, with Today and the arrows', async ({ backend, page }) => {
  await setUp(backend, page)
  await page.getByRole('button', { name: 'Next' }).click()
  await expect(page.getByTestId('cal-heading')).toHaveText('5 – 11 Oct 2026')
  await page.getByTestId('cal-today').click()
  await expect(page.getByTestId('cal-heading')).toHaveText('28 Sep – 4 Oct 2026')

  await page.getByRole('button', { name: 'day', exact: true }).click()
  await expect(page.getByTestId('cal-col')).toHaveCount(1)
  await expect(page.getByTestId('cal-heading')).toContainText('3 Oct 2026')

  await page.getByRole('button', { name: 'month', exact: true }).click()
  await expect(page.getByTestId('cal-heading')).toHaveText('October 2026')
  const cell = page.locator('[data-cell="2026-10-01"]')
  await expect(cell).toContainText('Range task')
  await expect(cell).toContainText('Team standup')
  // a click on a day number opens that day
  await cell.getByRole('button', { name: '1', exact: true }).click()
  await expect(page.getByTestId('cal-col')).toHaveCount(1)
  await expect(page.getByTestId('cal-heading')).toContainText('1 Oct 2026')
})

test('calendars have colours and can be hidden; the colour is kept in the graph', async ({
  backend,
  page,
}) => {
  const birthdays = await setUp(backend, page)
  const first = page.getByTestId('cal-item').filter({ hasText: 'Team standup' }).first()
  const before = await first.evaluate((el) => getComputedStyle(el).borderLeftColor)
  expect(before).toMatch(/^rgb/)

  // a chosen colour is used at once and written into the graph's config
  await page.getByLabel('Colour of Birthdays').fill('#ff0000')
  await expect(first).toHaveCSS('border-left-color', 'rgb(255, 0, 0)')
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('#ff0000')
  expect(
    (await backend.call<{ calendar_colors: Record<string, string> }>('calendar_status')).calendar_colors[
      birthdays
    ],
  ).toBe('#ff0000')

  // the legend hides a calendar, and the tasks and events switches hide their kind
  await page.getByLabel('Show Birthdays').uncheck()
  await expect(page.getByTestId('cal-item').filter({ hasText: 'Team standup' })).toHaveCount(0)
  await page.getByLabel('Show Birthdays').check()
  await expect(page.getByTestId('cal-item').filter({ hasText: 'Team standup' })).toHaveCount(7)
  await page.getByRole('checkbox', { name: 'Events' }).uncheck()
  await expect(page.getByTestId('cal-item').filter({ hasText: 'Team standup' })).toHaveCount(0)
  await expect(item(page, 'Range task')).toBeVisible()
})

/** Drags with the pointer from the middle of `from` to `x, y`. */
async function dragTo(page: Page, from: ReturnType<typeof item>, x: number, y: number, grabAt = 0.5) {
  const box = (await from.boundingBox())!
  await page.mouse.move(box.x + box.width / 2, box.y + box.height * grabAt)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height * grabAt + 10, { steps: 3 })
  await page.mouse.move(x, y, { steps: 8 })
  await page.mouse.up()
}

test('dragging moves a task, keeping its length; dragging the edge changes the end', async ({
  backend,
  page,
}) => {
  await setUp(backend, page)
  const friday = page.locator('[data-col="2026-10-02"]')
  const box = (await friday.boundingBox())!
  const at = (hour: number) => box.y + hour * 48

  // Thursday 09:00-10:30 grabbed in the middle (45 minutes below its top) and dropped so that its top is at 14:00
  await dragTo(page, item(page, 'Range task'), box.x + box.width * 0.5, at(14) + 36)
  await expect.poll(() => backend.read('pages/Plan.md')).toContain('scheduled:: 2026-10-02 14:00-15:30')

  // the view has caught up with the file (the blocks of Thursday are laid out anew) before the next measuring
  await expect(item(page, 'Range task')).toHaveAttribute('data-date', '2026-10-02')

  // the edge of "Short task" (09:00, one hour by default) is dragged to 11:30
  const short = item(page, 'Short task')
  const grip = short.getByRole('separator', { name: 'Change the end time' })
  const g = (await grip.boundingBox())!
  const thursday = (await page.locator('[data-col="2026-10-01"]').boundingBox())!
  await page.mouse.move(g.x + g.width / 2, g.y + g.height / 2)
  await page.mouse.down()
  await page.mouse.move(g.x + g.width / 2, thursday.y + 11.5 * 48, { steps: 8 })
  await page.mouse.up()
  await expect.poll(() => backend.read('pages/Plan.md')).toContain('scheduled:: 2026-10-01 09:00-11:30')
})

test('a task dropped on the all-day strip loses its time; one from the tray gets a date and an hour', async ({
  backend,
  page,
}) => {
  await setUp(backend, page)
  const strip = (await page.locator('[data-allday="2026-09-30"]').boundingBox())!
  await dragTo(page, item(page, 'Range task'), strip.x + strip.width / 2, strip.y + strip.height / 2)
  await expect.poll(() => backend.read('pages/Plan.md')).toContain('scheduled:: 2026-09-30\n')

  const wednesday = (await page.locator('[data-col="2026-09-30"]').boundingBox())!
  const tray = page.getByTestId('cal-tray-item').first()
  const t = (await tray.boundingBox())!
  await page.mouse.move(t.x + 10, t.y + t.height / 2)
  await page.mouse.down()
  await page.mouse.move(t.x - 20, t.y + 20, { steps: 3 })
  await page.mouse.move(wednesday.x + wednesday.width / 2, wednesday.y + 15 * 48 + 2, { steps: 8 })
  await expect(page.getByTestId('cal-ghost')).toBeVisible()
  await page.mouse.up()
  await expect
    .poll(() => backend.read('pages/Plan.md'))
    .toContain('- TODO Undated task\n  scheduled:: 2026-09-30 15:00-16:00')
  await expect(page.getByTestId('cal-tray-item')).toHaveCount(0)
})

test("a click on an empty slot makes a task of an hour in today's journal", async ({ backend, page }) => {
  await setUp(backend, page)
  const tuesday = (await page.locator('[data-col="2026-09-29"]').boundingBox())!
  await page.mouse.click(tuesday.x + tuesday.width / 2, tuesday.y + 10 * 48 + 5)
  const input = page.getByLabel('New task')
  await expect(input).toBeVisible()
  await input.fill('Call the dentist')
  await input.press('Enter')
  await expect
    .poll(() => backend.read('journals/2026/2026-10-03.md'))
    .toContain('TODO Call the dentist\n  scheduled:: 2026-09-29 10:00-11:00')
  // and it shows in the grid
  await expect(item(page, 'Call the dentist')).toHaveAttribute('data-start', '600')
})

test('an event opens its details; a task opens its note; the checkbox completes a task', async ({
  backend,
  page,
}) => {
  await setUp(backend, page)
  await page.getByTestId('cal-item').filter({ hasText: 'Team standup' }).first().click()
  const popup = page.getByTestId('cal-event-popup')
  await expect(popup).toContainText('Team standup')
  await expect(popup).toContainText('09:00–09:15')
  await popup.getByRole('button', { name: 'Close' }).click()
  await expect(popup).toHaveCount(0)

  await item(page, 'Short task').getByRole('checkbox').check()
  await expect.poll(() => backend.read('pages/Plan.md')).toContain('DONE Short task')
})

test('overdue tasks can be shown rolled over to today or at their dates; nothing is written', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/Late.md',
    [
      '- TODO Late timed',
      '  scheduled:: 2026-10-01 09:00',
      '- TODO Late deadline',
      '  deadline:: 2026-09-29',
      '- DONE Old done',
      '  scheduled:: 2026-10-01 11:00',
      '- TODO Future',
      '  scheduled:: 2026-10-04 10:00',
      '',
    ].join('\n'),
  )
  await setUp(backend, page)
  const files = () => `${backend.read('pages/Late.md')}|${backend.read('pages/Plan.md')}`
  const before = files()
  const badges = page.getByTestId('cal-rolled')

  // at their dates (the default): every task is on the day it was planned for
  await expect(badges).toHaveCount(0)
  await expect(item(page, 'Late timed')).toHaveAttribute('data-date', '2026-10-01')
  await expect(item(page, 'Late timed')).toHaveAttribute('data-start', '540')

  // rolled over: the open overdue ones sit in today's strip with their age; the done and the future one stay
  await page.getByLabel('Overdue tasks').selectOption('today')
  const today = page.locator('[data-allday="2026-10-03"]')
  await expect(today.locator('[data-testid="cal-item"]', { hasText: 'Late timed' })).toContainText('↻ 2d')
  await expect(today.locator('[data-testid="cal-item"]', { hasText: 'Late deadline' })).toContainText('↻ 4d')
  // the tasks of the plan page are late too (the fixture graph may have more, so no total is counted)
  await expect(today.locator('[data-testid="cal-item"]', { hasText: 'Range task' })).toContainText('↻ 2d')
  await expect(today.locator('[data-testid="cal-item"]', { hasText: 'Day task' })).toContainText('↻ 1d')
  // finished tasks are only listed with "Show done", and they stay where they were done
  await page.getByTestId('agenda-done').check()
  await expect(item(page, 'Old done')).toHaveAttribute('data-date', '2026-10-01')
  await expect(item(page, 'Future')).toHaveAttribute('data-date', '2026-10-04')
  expect(files()).toBe(before)

  // the choice is kept; a week that does not include today shows what was planned when
  await page.reload()
  await backend.open('/agenda')
  await expect(page.getByLabel('Overdue tasks')).toHaveValue('today')
  await page.getByRole('button', { name: 'Previous' }).click()
  await expect(page.getByTestId('cal-heading')).toHaveText('21 – 27 Sep 2026')
  await expect(badges).toHaveCount(0)

  // dragging a rolled task gives it a real new date
  await page.getByTestId('cal-today').click()
  const strip = (await page.locator('[data-allday="2026-10-04"]').boundingBox())!
  const rolled = today.locator('[data-testid="cal-item"]', { hasText: 'Late deadline' })
  const box = (await rolled.boundingBox())!
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + 10, { steps: 3 })
  await page.mouse.move(strip.x + strip.width / 2, strip.y + strip.height / 2, { steps: 8 })
  await page.mouse.up()
  await expect.poll(() => backend.read('pages/Late.md')).toContain('deadline:: 2026-10-04')
})
