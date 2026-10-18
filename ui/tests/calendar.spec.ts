import { expect, test } from './fixtures'

// Settings → Calendar against the dev server's in-memory calendar server (CA-1, CA-2).
test.use({ fakeCalendar: true })

test('connect with a login of its own, choose calendars, and save', async ({ backend, page }) => {
  await backend.open('/settings')
  const dialog = page
  await dialog.getByRole('tab', { name: 'Calendar' }).click()

  // no sync login to borrow: the form for a login is shown
  const form = dialog.getByTestId('calendar-login')
  await form.getByLabel('Server').fill('http://localhost:8080')
  await form.getByLabel('User name').fill('me')
  await form.getByLabel(/^Password/).fill('secret')
  await form.getByRole('button', { name: 'Connect' }).click()
  await expect(dialog.getByText('3 calendars found')).toBeVisible()

  // a calendar that takes no tasks cannot be chosen for them
  await expect(dialog.getByLabel('Tasks go to Birthdays')).toBeDisabled()
  await expect(dialog.getByLabel('Show events of Tasks')).toBeDisabled()
  await dialog.getByLabel('Tasks go to Tasks').check()
  await dialog.getByLabel('Show events of Personal').check()
  await dialog.getByLabel('Calendar sync is on').check()
  await dialog.getByTestId('calendar-save').click()

  await expect
    .poll(async () => (await backend.call<{ enabled: boolean; read: string[] }>('calendar_status')).enabled)
    .toBe(true)
  const st = await backend.call<{ tasks: string; read: string[]; account: { user: string } }>(
    'calendar_status',
  )
  expect(st.tasks).toContain('tasks')
  expect(st.read).toHaveLength(1)
  expect(st.account.user).toBe('me')

  // dated tasks are sent; the fixture's first sync gives them block IDs
  await dialog.getByTestId('calendar-sync').click()
  await expect(dialog.getByTestId('calendar-sync-result')).toContainText('sent')
  await dialog.getByTestId('calendar-sync').click()
  await expect(dialog.getByTestId('calendar-sync-result')).toContainText('up to date')
})

test('the import of tasks made in other apps is a switch that is saved at once', async ({
  backend,
  page,
}) => {
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Calendar' }).click()
  const form = page.getByTestId('calendar-login')
  await form.getByLabel('Server').fill('http://localhost:8080')
  await form.getByLabel('User name').fill('me')
  await form.getByLabel(/^Password/).fill('secret')
  await form.getByRole('button', { name: 'Connect' }).click()
  await page.getByLabel('Tasks go to Tasks').check()
  await page.getByLabel('Calendar sync is on').check()
  await page.getByTestId('calendar-save').click()
  const box = page.getByTestId('calendar-import')
  await expect(box).not.toBeChecked()
  await box.check()
  await expect
    .poll(async () => (await backend.call<{ import_tasks: boolean }>('calendar_status')).import_tasks)
    .toBe(true)
  await box.uncheck()
  await expect
    .poll(async () => (await backend.call<{ import_tasks: boolean }>('calendar_status')).import_tasks)
    .toBe(false)
})

test('with calendar sync on, the app syncs by itself when the graph opens', async ({ backend, page }) => {
  await backend.open('/')
  const list = await backend.call<{ calendars: { href: string; name: string }[] }>('calendar_connect', {
    server: 'http://localhost:8080',
    user: 'me',
    password: 'secret',
  })
  const tasks = list.calendars.find((c) => c.name === 'Tasks')!.href
  await backend.call('calendar_save', { enabled: true, read: [], tasks, import_tasks: false })
  await page.reload()
  await page.getByRole('link', { name: 'Settings' }).click()
  await page.getByRole('tab', { name: 'Calendar' }).click()
  await expect(page.getByTestId('calendar-auto')).toContainText('Last synced', { timeout: 15_000 })
})

test('events of the chosen calendars show in the journal and in the timeline, and the notes stay as they are', async ({
  backend,
  page,
}) => {
  await backend.open('/')
  const list = await backend.call<{ calendars: { href: string; name: string }[] }>('calendar_connect', {
    server: 'http://localhost:8080',
    user: 'me',
    password: 'secret',
  })
  const birthdays = list.calendars.find((c) => c.name === 'Birthdays')!.href
  await backend.call('calendar_save', { enabled: true, read: [birthdays], tasks: null, import_tasks: false })
  await backend.call('calendar_sync')
  await page.reload()
  const day = page.getByTestId('day-events').first()
  await expect(day).toContainText('Team standup')
  await expect(day).toContainText('09:00–09:15')
  await expect(day.getByRole('heading', { name: 'Calendar' })).toBeVisible()

  // the timeline of a query shows the same events beside the tasks
  backend.write(
    'pages/cal.md',
    '- TODO plan ^pl1\n  deadline:: 2026-10-12\n- ```query\n  task: todo\n  page: cal\n  view: timeline\n  ```\n',
  )
  await backend.open('/page/cal')
  await expect(page.getByTestId('cal-event').first()).toContainText('Team standup')
})
