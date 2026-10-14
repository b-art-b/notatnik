import { expect, test } from './fixtures'

// TK-2: a scheduled date or a deadline can carry a start time and an end time; off by default.
import type { Page } from '@playwright/test'

const chip = (page: Page, key: string) =>
  page.getByRole('tree', { name: 'Blocks' }).locator(`[data-chip="${key}"]`).first()
const PAGE = '- TODO plan\n  scheduled:: 2026-10-05\n  deadline:: 2026-10-09\n'

test('a time is off by default; with it the value gets a start and an end', async ({ backend, page }) => {
  backend.write('pages/plan.md', PAGE)
  await backend.open('/page/plan')
  await chip(page, 'scheduled').click()
  const picker = page.getByTestId('date-picker')
  // a plain date: no time controls shown until asked for
  await expect(picker.getByRole('checkbox', { name: 'Time of day' })).not.toBeChecked()
  await expect(picker.getByTestId('time-row')).toHaveCount(0)
  await picker.getByRole('checkbox', { name: 'Time of day' }).check()
  await picker.getByLabel('Start time (24 h, HH:MM)').fill('9:30')
  await picker.getByRole('button', { name: '1 h' }).click()
  await expect(picker.getByLabel('End time (optional, HH:MM)')).toHaveValue('10:30')
  await picker.getByRole('button', { name: 'Previous month' }).click() // moving the calendar changes nothing
  await picker.getByRole('button', { name: 'Next month' }).click()
  await picker.getByLabel('2026-10-06').click()
  await expect.poll(() => backend.read('pages/plan.md')).toContain('scheduled:: 2026-10-06 09:30-10:30')
  // the chip shows it, and the deadline is untouched
  await expect(chip(page, 'scheduled')).toContainText('2026-10-06 09:30-10:30')
  expect(backend.read('pages/plan.md')).toContain('deadline:: 2026-10-09\n')
})

test('an end before the start is refused; a start alone and a plain date are fine', async ({
  backend,
  page,
}) => {
  backend.write('pages/plan.md', PAGE)
  await backend.open('/page/plan')
  await chip(page, 'deadline').click()
  const picker = page.getByTestId('date-picker')
  await picker.getByRole('checkbox', { name: 'Time of day' }).check()
  await picker.getByLabel('Start time (24 h, HH:MM)').fill('17:00')
  await picker.getByLabel('End time (optional, HH:MM)').fill('16:00')
  await expect(picker.getByRole('alert')).toContainText('after the start')
  await picker.getByLabel('2026-10-10').click()
  expect(backend.read('pages/plan.md')).toContain('deadline:: 2026-10-09\n')
  // without an end: a time point
  await picker.getByRole('button', { name: 'No end' }).click()
  await expect(picker.getByRole('alert')).toHaveCount(0)
  await picker.getByLabel('2026-10-10').click()
  await expect.poll(() => backend.read('pages/plan.md')).toContain('deadline:: 2026-10-10 17:00\n')
  // opened again it shows the time; switching it off writes a plain date
  await chip(page, 'deadline').click()
  await expect(picker.getByRole('checkbox', { name: 'Time of day' })).toBeChecked()
  await expect(picker.getByLabel('Start time (24 h, HH:MM)')).toHaveValue('17:00')
  await picker.getByRole('checkbox', { name: 'Time of day' }).uncheck()
  await picker.getByLabel('2026-10-10').click()
  await expect.poll(() => backend.read('pages/plan.md')).toContain('deadline:: 2026-10-10\n')
})

test('a range written by hand is shown, and the timeline marks the time', async ({ backend, page }) => {
  backend.write(
    'pages/cal.md',
    '- TODO standup\n  deadline:: 2026-10-12 09:00 – 09:15\n- ```query\n  task: todo\n  page: cal\n  view: timeline\n  ```\n',
  )
  await backend.open('/page/cal')
  await expect(chip(page, 'deadline')).toContainText('2026-10-12 09:00-09:15')
  const day = page.locator('td[data-date="2026-10-12"]')
  await expect(day.getByTestId('ev-time')).toHaveText('09:00–09:15')
  // moving the task to another day keeps the times
  await day.locator('[data-ev]').first().dragTo(page.locator('td[data-date="2026-10-13"]'))
  await expect.poll(() => backend.read('pages/cal.md')).toContain('deadline:: 2026-10-13 09:00-09:15')
})
