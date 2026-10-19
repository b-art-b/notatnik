import { expect, test } from './fixtures'

test('a page lists its tasks: on the page, in subpages and elsewhere mentioning it', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/Acme.md',
    '- Notes\n  - TODO own open task\n  - DONE own finished task\n- plain text\n',
  )
  backend.write('pages/Acme/support.md', '- DOING subpage task\n')
  backend.write(
    'pages/notes.md',
    '- TODO call [[Acme]] back\n- DONE sent the offer to [[Acme]]\n- TODO unrelated task\n',
  )
  await backend.open('/page/Acme')
  const section = page.getByTestId('page-tasks')
  await expect(section).toBeVisible()
  await expect(page.getByTestId('page-tasks-count')).toHaveText('3')
  const own = section.locator('[data-group="own"]')
  await expect(own).toContainText('own open task')
  await expect(own).not.toContainText('own finished task') // done tasks are hidden until asked for
  await expect(section.locator('[data-group="sub"]')).toContainText('subpage task')
  const mentions = section.locator('[data-group="mentions"]')
  await expect(mentions).toContainText('call')
  await expect(section).not.toContainText('unrelated task')
  // done tasks on request
  await page.getByTestId('page-tasks-done').check()
  await expect(page.getByTestId('page-tasks-count')).toHaveText('5')
  await expect(own).toContainText('own finished task')
  await expect(mentions).toContainText('sent the offer')
  await page.getByTestId('page-tasks-done').uncheck()
  // the list is read again a moment after the switch: wait until the done task is gone before using it
  await expect(mentions).not.toContainText('sent the offer')
  await expect(page.getByTestId('page-tasks-count')).toHaveText('3')
  // a task can be changed right there and the file follows
  await mentions.locator('[data-task="TODO"]').click()
  await expect.poll(() => backend.read('pages/notes.md')).toContain('- DOING call [[Acme]] back')
  // the source link opens the note and flashes the task
  await expect(mentions.locator('[data-task="DOING"]')).toBeVisible()
  await mentions.getByTestId('page-task-source').click()
  await expect(page).toHaveURL(/#\/page\/notes/)
  await expect(page.locator('.block.flash')).toContainText('call')
})

test('a page without tasks shows no tasks section', async ({ backend, page }) => {
  backend.write('pages/Quiet.md', '- nothing to do here\n')
  await backend.open('/page/Quiet')
  await expect(page.getByText('nothing to do here')).toBeVisible()
  await expect(page.getByTestId('page-tasks')).toHaveCount(0)
})

test('a task below a bullet that links the page counts as mentioning it', async ({ backend, page }) => {
  backend.write('pages/Client One.md', '- the client\n')
  backend.write(
    'pages/meeting.md',
    '- [[Client One]]:\n  - TODO prepare offer\n    - TODO check prices\n- TODO unrelated task\n',
  )
  await backend.open('/page/Client One')
  const section = page.getByTestId('page-tasks')
  await expect(section).toContainText('prepare offer')
  await expect(section).toContainText('check prices')
  await expect(section).not.toContainText('unrelated task')
})
