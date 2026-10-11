import { expect, test } from './fixtures'

test('/todo typed anywhere in the line sets the marker at the line start', async ({ backend, page }) => {
  backend.write('pages/t.md', '- buy milk\n')
  await backend.open('/page/t')
  await page.getByText('buy milk').click()
  const editor = page.getByTestId('block-editor')
  await expect(editor).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /doing')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Mark as doing/) // arrives async
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await page.keyboard.press('Enter')
  await expect(editor).toContainText('DOING buy milk')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- DOING buy milk\n')
  await page.keyboard.type(' /done')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Mark as done/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect.poll(() => backend.read('pages/t.md')).toMatch(/^- DONE buy milk\n/)
})

test('/deadline opens a date picker and writes the property', async ({ backend, page }) => {
  backend.write('pages/t.md', '- TODO pay rent\n')
  await backend.open('/page/t')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('pay rent').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /deadline')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Set deadline/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  const picker = page.getByTestId('date-picker')
  await expect(picker).toBeVisible()
  await picker.getByRole('textbox', { name: /Date/ }).fill('2026-12-24')
  await expect(picker).toContainText('2026-12-24')
  await page.keyboard.press('Enter')
  await expect(picker).toBeHidden()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await expect.poll(() => backend.read('pages/t.md')).toBe('- TODO pay rent\n  deadline:: 2026-12-24\n')
})

test('/date inserts a link at the cursor; Escape cancels', async ({ backend, page }) => {
  backend.write('pages/t.md', '- call\n')
  await backend.open('/page/t')
  await page.getByText('call').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /date')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Pick a date/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await page.getByRole('button', { name: 'tomorrow' }).click()
  await expect(page.getByTestId('block-editor')).toContainText(/call \[\[\d{4}-\d{2}-\d{2}\]\]/)
  await page.keyboard.type(' /date')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Pick a date/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('date-picker')).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('date-picker')).toBeHidden()
  await expect(page.getByTestId('block-editor')).toBeFocused()
})

test('date picker has an ISO calendar: navigate months and click a day', async ({ backend, page }) => {
  backend.write('pages/t.md', '- TODO pay rent\n')
  await backend.open('/page/t')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('pay rent').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /deadline')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Set deadline/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  const picker = page.getByTestId('date-picker')
  await picker.getByRole('textbox').fill('2026-12-05')
  await expect(picker.getByTestId('picker-month')).toHaveText('2026-12')
  await picker.getByRole('button', { name: 'Next month' }).click()
  await expect(picker.getByTestId('picker-month')).toHaveText('2027-01')
  await picker.getByRole('button', { name: '2027-01-15' }).click()
  await expect(picker).toBeHidden()
  await expect.poll(() => backend.read('pages/t.md')).toBe('- TODO pay rent\n  deadline:: 2027-01-15\n')
})

test('/log adds dated entries to a logbook that is listed, not rendered', async ({ backend, page }) => {
  backend.write('pages/t.md', '- DOING write spec\n  deadline:: 2026-10-10\n')
  await backend.open('/page/t')
  const blocks = page.getByRole('tree', { name: 'Blocks' }) // the Tasks section below shows the task too
  await blocks.getByText('write spec').click()
  const editor = page.getByTestId('block-editor')
  await expect(editor).toBeFocused()
  await page.keyboard.press('Control+End')
  await page.keyboard.type(' /log')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Log progress/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await page.keyboard.type('outline done')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toMatch(
      /^- DOING write spec\n {2}deadline:: 2026-10-10\n {2}:LOGBOOK:\n {2}2026-10-03 \d\d:\d\d outline done\n {2}:END:\n$/,
    )
  // rendered: a collapsible list with the entry; the drawer markers are not shown as text
  const log = blocks.getByTestId('logbook')
  // collapsed: the latest entry is already visible
  await expect(log).toContainText('Logbook:')
  await expect(log).toContainText('outline done')
  await log.getByRole('button', { name: 'Logbook:' }).click()
  await expect(log.getByRole('list', { name: /Logbook entries/ })).toContainText('outline done')
  await expect(log).toContainText('1 entry')
  await expect(blocks.locator('.nodes')).not.toContainText('LOGBOOK')
})

test('logbook notes render links and tags, and a link opens the page', async ({ backend, page }) => {
  backend.write(
    'pages/t.md',
    '- DOING write spec\n  :LOGBOOK:\n  2026-10-04 14:30 talked to [[Anna]] about **scope** #urgent\n  :END:\n',
  )
  backend.write('pages/Anna.md', '- Anna page\n')
  await backend.open('/page/t')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  const log = blocks.getByTestId('logbook')
  // the preview line renders the note too; open the whole list
  await expect(log.locator('a.page-ref[data-page="Anna"]')).toBeVisible()
  await log.getByRole('button', { name: 'Logbook:' }).click()
  await expect(log.getByRole('list')).toBeVisible()
  await expect(log.locator('strong')).toHaveText('scope')
  await expect(log.locator('a.tag[data-page="urgent"]')).toBeVisible()
  await expect(log).not.toContainText('[[')
  await log.locator('a.page-ref[data-page="Anna"]').click()
  await expect(page).toHaveURL(/#\/page\/Anna/)
})

test('block commands typed mid-text put their result in the right place', async ({ backend, page }) => {
  backend.write(
    'pages/t.md',
    '- write the report\n  some body text\n  :LOGBOOK:\n  2026-10-01 started\n  :END:\n',
  )
  await backend.open('/page/t')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('write the report').click()
  const editor = page.getByTestId('block-editor')
  await expect(editor).toBeFocused()
  // in the middle of the body text, not at the end
  await page.keyboard.press('Control+Home')
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('End')
  await page.keyboard.type(' /priority')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Priority A/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  // the property sits under the first line, above the body and the logbook
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe(
      '- write the report\n  priority:: A\n  some body text\n  :LOGBOOK:\n  2026-10-01 started\n  :END:\n',
    )
  // and /todo from the same place marks the first line
  await page.keyboard.type(' /todo')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Make this a task/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect.poll(() => backend.read('pages/t.md')).toMatch(/^- TODO write the report\n {2}priority:: A\n/)
})

test('the logbook row previews the latest entry and opens to a timeline, newest first', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/t.md',
    '- DOING write spec\n  scheduled:: 2026-10-10\n  :LOGBOOK:\n  2026-10-01 10:00 outline\n  2026-10-02 11:00 first draft\n  2026-10-03 12:00 review done\n  :END:\n',
  )
  await backend.open('/page/t')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  const log = blocks.getByTestId('logbook').first()
  await expect(log).toContainText('review done') // the latest, without a click
  await expect(log).not.toContainText('outline')
  await expect(log).toContainText('+2 earlier')
  // it is the row right after SCHEDULED, in the same label style
  const labels = await blocks
    .locator('.chips .lab, .logbook .lab')
    .first()
    .evaluate((el) => getComputedStyle(el).textTransform)
  expect(labels).toBe('uppercase')
  await log.getByRole('button', { name: '+2 earlier' }).click()
  const items = log.getByRole('listitem')
  await expect(items).toHaveCount(3)
  await expect(items.first()).toContainText('review done')
  await expect(items.last()).toContainText('outline')
  await expect(log.getByRole('button', { name: 'Logbook:' })).toHaveAttribute('aria-expanded', 'true')
  // remembered on this device
  await page.reload()
  await expect(blocks.getByTestId('logbook').first().getByRole('listitem')).toHaveCount(3)
})

test('a / command typed in the middle of a word changes metadata only', async ({ backend, page }) => {
  backend.write('pages/t.md', '- buy milk\n')
  await backend.open('/page/t')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('buy milk').click()
  const editor = page.getByTestId('block-editor')
  await expect(editor).toBeFocused()
  await page.keyboard.press('End')
  // `/d` right after a letter: only metadata commands are offered, not the ones that insert text
  await page.keyboard.type(' some wor/tod')
  const menu = page.locator('.cm-tooltip-autocomplete')
  await expect(menu).toHaveText(/Make this a task/)
  await expect(menu).not.toContainText('Today')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  // the command is gone from the word, the marker is at the start of the line
  await expect(editor).toContainText('TODO buy milk some wor')
  await expect(editor).not.toContainText('/tod')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- TODO buy milk some wor\n')
  // after punctuation it is the full menu again
  await page.keyboard.type(' (/tod')
  await expect(menu).toContainText('Today')
  await page.keyboard.press('Escape')
})

test('slashes in URLs and paths do not open the menu', async ({ backend, page }) => {
  backend.write('pages/t.md', '- link\n')
  await backend.open('/page/t')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('link').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' https://example.com/done and a/b km/h')
  await page.waitForTimeout(300)
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveCount(0)
})
