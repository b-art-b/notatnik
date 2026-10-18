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
