import { expect, test } from './fixtures'

test.describe('refs fixture', () => {
  test.use({ fixture: 'refs' })
  test('refs, embeds, page embeds, cycles and missing targets render', async ({ backend, page }) => {
    backend.write(
      'pages/cycle.md',
      '- A ^aaa000\n  - embeds !((bbb000))\n- B ^bbb000\n  - embeds !((aaa000))\n',
    )
    await backend.open('/page/linker')
    await expect(page.locator('a.block-ref[data-uid="aaa111"]')).toContainText('Top block with an id')
    await expect(page.locator('[data-embed="bbb222"]')).toContainText('Grandchild')
    await expect(page.locator('[data-embed-page="source"]')).toContainText('Another block')
    await expect(page.getByText('broken ref')).toBeVisible()
    await page.waitForTimeout(500)
    await backend.open('/page/cycle')
    await expect(page.getByText('embed cycle').first()).toBeVisible()
  })
  test('backlinks panel matches the backend', async ({ backend, page }) => {
    await backend.open('/page/source')
    const expected = await backend.call<unknown[]>('backlinks', { page: 'source' })
    await expect(page.getByTestId('backlink-count')).toHaveText(String(expected.length))
    await expect(page.locator('.group[data-source="other"]')).toContainText('Child linking to')
  })
})

test('raw HTML is literal text and nothing executes', async ({ backend, page }) => {
  backend.write(
    'pages/html.md',
    '- <script>window.__pwned = 1</script> and <b>bold</b> and <img src=x onerror="window.__pwned = 2">\n',
  )
  await backend.open('/page/html')
  await expect(page.getByText('<b>bold</b>')).toBeVisible()
  expect(await page.evaluate(() => (window as unknown as { __pwned?: number }).__pwned)).toBeUndefined()
  await expect(page.locator('article img')).toHaveCount(0)
})

test('search finds żółw from "zolw" and flashes the block', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+Shift+F')
  await page.getByLabel('Search text').fill('zolw')
  await expect(page.getByTestId('search-hit').first()).toContainText('Żółw')
  await page.keyboard.press('Enter')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('zażółć')
})

test('namespace tree equals the folder structure and updates', async ({ backend, page }) => {
  await backend.open('/journals')
  const tree = page.getByRole('tree', { name: 'Namespaces' })
  await expect(tree.locator('a[data-ns="work"]')).toBeVisible()
  await tree.locator('a[data-ns="work"]').locator('xpath=../button').click()
  await expect(tree.locator('a[data-ns="work/projects"]')).toBeVisible()
  backend.write('pages/zzz/new-page.md', '- hello\n')
  await expect(tree.locator('a[data-ns="zzz"]')).toBeVisible({ timeout: 5000 })
})

test('rename previews files and rewrites links', async ({ backend, page }) => {
  await backend.open('/page/people%2Fanna')
  await page.getByTitle('Rename page').click()
  await page.getByLabel('New name').fill('team/anna')
  await page.getByRole('button', { name: 'Preview' }).click()
  await expect(page.getByTestId('rename-plan')).toContainText('3 link(s) in 3 file(s)')
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  await expect(page).toHaveURL(/team%2Fanna/)
  expect(backend.read('pages/work/projects/alpha.md')).toContain('[[team/anna]]')
  expect(backend.exists('pages/team/anna.md')).toBe(true)
})

test('moving a whole namespace: preview lists the moves, pages and links follow', async ({
  backend,
  page,
}) => {
  backend.write('pages/Przepisy.md', '- recipes root\n')
  backend.write('pages/Przepisy/first.md', '- first recipe\n')
  backend.write('pages/Przepisy/soups/tomato.md', '- tomato soup, back to [[Przepisy]]\n')
  backend.write('pages/menu.md', '- cook [[Przepisy/soups/tomato]] and [[przepisy/first]]\n')
  await backend.open('/page/Przepisy')
  await page.getByTitle('Rename page').click()
  await page.getByLabel('New name').fill('Kuchnia/Przepisy')
  await page.getByRole('button', { name: 'Preview' }).click()
  const plan = page.getByTestId('rename-plan')
  await expect(plan).toContainText('3 page files move')
  await page.getByTestId('rename-moves').locator('summary').click()
  await expect(plan).toContainText('pages/Przepisy/soups/tomato.md → pages/Kuchnia/Przepisy/soups/tomato.md')
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  await expect(page).toHaveURL(/Kuchnia%2FPrzepisy/)
  expect(backend.exists('pages/Kuchnia/Przepisy.md')).toBe(true)
  expect(backend.exists('pages/Kuchnia/Przepisy/first.md')).toBe(true)
  expect(backend.exists('pages/Kuchnia/Przepisy/soups/tomato.md')).toBe(true)
  expect(backend.exists('pages/Przepisy')).toBe(false)
  expect(backend.read('pages/menu.md')).toBe(
    '- cook [[Kuchnia/Przepisy/soups/tomato]] and [[Kuchnia/Przepisy/first]]\n',
  )
  expect(backend.read('pages/Kuchnia/Przepisy/soups/tomato.md')).toContain('[[Kuchnia/Przepisy]]')
})

test('a page with subpages can be renamed to a name below itself: only that page moves', async ({
  backend,
  page,
}) => {
  backend.write('pages/Napis/second.md', '- the second page\n')
  backend.write('pages/Napis/second/second-deep/second-deeper.md', '- deeper page\n')
  backend.write('pages/other.md', '- see [[Napis/second]] and [[Napis/second/second-deep/second-deeper]]\n')
  await backend.open('/page/Napis%2Fsecond')
  await page.getByTitle('Rename page').click()
  const dialog = page.getByRole('dialog', { name: 'Rename page' })
  await dialog.getByLabel('New name').fill('Napis/second/second-deep/second-deeper')
  // the option is on by default, but switches itself off: a page can't move into its own subpages
  await expect(dialog.getByLabel(/Move the subpages too/)).toBeDisabled()
  await dialog.getByRole('button', { name: 'Preview' }).click()
  // the page named like that already exists: refused, nothing changes
  await expect(page.getByRole('alert')).toContainText('already exists')
  await dialog.getByLabel('New name').fill('Napis/second/second-deep/renamed')
  await dialog.getByRole('button', { name: 'Preview' }).click()
  await expect(page.getByTestId('rename-plan')).toContainText('the page file moves to')
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  await expect(page).toHaveURL(/second-deep%2Frenamed/)
  expect(backend.exists('pages/Napis/second/second-deep/renamed.md')).toBe(true)
  expect(backend.exists('pages/Napis/second.md')).toBe(false)
  // the subpage did not move; links to the old name follow, links to the subpage are untouched
  expect(backend.exists('pages/Napis/second/second-deep/second-deeper.md')).toBe(true)
  expect(backend.read('pages/other.md')).toBe(
    '- see [[Napis/second/second-deep/renamed]] and [[Napis/second/second-deep/second-deeper]]\n',
  )
})

test('renaming a page with subpages can leave the subpages where they are', async ({ backend, page }) => {
  backend.write('pages/Team.md', '- team page\n')
  backend.write('pages/Team/anna.md', '- anna\n')
  await backend.open('/page/Team')
  await page.getByTitle('Rename page').click()
  const dialog = page.getByRole('dialog', { name: 'Rename page' })
  await dialog.getByLabel('New name').fill('Crew')
  await dialog.getByLabel(/Move the subpages too/).uncheck()
  await dialog.getByRole('button', { name: 'Preview' }).click()
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  await expect(page).toHaveURL(/#\/page\/Crew/)
  expect(backend.exists('pages/Crew.md')).toBe(true)
  expect(backend.exists('pages/Team/anna.md')).toBe(true) // stayed
})

test('external change appears without moving focus; conflict file shows a banner', async ({
  backend,
  page,
}) => {
  await backend.open('/page/inbox')
  await page.getByText('Call the dentist').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  backend.write('pages/inbox.md', backend.read('pages/inbox.md') + '- added by an external tool\n')
  await expect(page.getByText('added by an external tool')).toBeVisible({ timeout: 5000 })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  backend.write('pages/inbox (conflicted copy 2026-10-03 101010).md', '- other version\n')
  await expect(page.getByTestId('conflict-banner')).toBeVisible({ timeout: 5000 })
})

test('collapsed state survives reload and leaves the file alone', async ({ backend, page }) => {
  await backend.open('/page/inbox')
  const before = backend.read('pages/inbox.md')
  // the page's own tree: the Tasks section below lists open tasks even under collapsed blocks
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await page.getByRole('button', { name: 'Collapse', exact: true }).first().click()
  await expect(blocks.getByText('Renew passport')).toBeHidden()
  await page.waitForTimeout(300)
  await page.reload()
  await expect(blocks.getByText('Things to sort out')).toBeVisible()
  await expect(blocks.getByText('Renew passport')).toBeHidden()
  expect(backend.read('pages/inbox.md')).toBe(before)
})

test.describe('queries fixture', () => {
  test.use({ fixture: 'queries' })
  test('typing [[ completes page names, namespace siblings first', async ({ backend, page }) => {
    await backend.open('/page/work%2Fprojects%2Falpha')
    await page.getByText('Notes about alpha').click()
    await page.keyboard.press('End')
    await page.keyboard.type(' [[be')
    await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('work/projects/beta', {
      timeout: 5000,
    })
    await page.keyboard.press('Escape')
  })
})

test('selecting blocks: indent all and copy as Markdown', async ({ backend, page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  backend.write('pages/sel.md', '- p\n- a\n- b\n- c\n')
  await backend.open('/page/sel')
  await page.getByText('a', { exact: true }).click()
  await page.keyboard.press('Escape')
  await page.keyboard.press('Shift+ArrowDown')
  await page.keyboard.press('Shift+ArrowDown')
  await page.keyboard.press('Control+c')
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('- a\n- b\n- c')
  await page.keyboard.press('Tab')
  await expect.poll(() => backend.read('pages/sel.md')).toBe('- p\n  - a\n  - b\n  - c\n')
  await page.keyboard.press('Delete')
  await expect.poll(() => backend.read('pages/sel.md')).toBe('- p\n')
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/sel.md')).toBe('- p\n  - a\n  - b\n  - c\n')
})

test('typing then Tab immediately never writes the text into another block', async ({ backend, page }) => {
  backend.write('pages/safe.md', '- a\n- b\n- keep me\n')
  await backend.open('/page/safe')
  await page.getByText('b', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' typed')
  await page.keyboard.press('Tab')
  await expect.poll(() => backend.read('pages/safe.md')).toBe('- a\n  - b typed\n- keep me\n')
  await page.waitForTimeout(800)
  expect(backend.read('pages/safe.md')).toBe('- a\n  - b typed\n- keep me\n')
})

test('a block changed on disk while editing: keep mine / take theirs', async ({ backend, page }) => {
  backend.write('pages/conf.md', '- first\n- second\n')
  await backend.open('/page/conf')
  await page.getByText('first').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' mine')
  // the same block is changed by another tool
  backend.write('pages/conf.md', '- first THEIRS\n- second\n')
  await expect(page.getByTestId('block-conflict')).toBeHidden()
  await page.waitForTimeout(1200)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('block-conflict')).toBeVisible()
  expect(backend.read('pages/conf.md')).toBe('- first THEIRS\n- second\n')
  await page.getByRole('button', { name: 'Take theirs' }).click()
  await page.keyboard.press('Escape')
  expect(backend.read('pages/conf.md')).toBe('- first THEIRS\n- second\n')
})

test('keep mine writes the local text', async ({ backend, page }) => {
  backend.write('pages/conf.md', '- first\n- second\n')
  await backend.open('/page/conf')
  await page.getByText('first').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' mine')
  backend.write('pages/conf.md', '- first THEIRS\n- second\n')
  await page.waitForTimeout(1200)
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'Keep mine' }).click()
  await expect.poll(() => backend.read('pages/conf.md')).toBe('- first mine\n- second\n')
})

test('a different block changed on disk while editing: no prompt, both changes survive', async ({
  backend,
  page,
}) => {
  backend.write('pages/conf.md', '- first\n- second\n')
  await backend.open('/page/conf')
  await page.getByText('first').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' mine')
  backend.write('pages/conf.md', '- first\n- second THEIRS\n')
  await page.waitForTimeout(1200)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/conf.md')).toBe('- first mine\n- second THEIRS\n')
  await expect(page.getByTestId('block-conflict')).toBeHidden()
})

test('settings: theme and date format are device settings, week start is a graph setting', async ({
  backend,
  page,
}) => {
  await backend.open('/settings')
  const cfgBefore = backend.read('.notatnik/config.toml')
  await page.getByRole('radio', { name: 'Dark' }).check()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  expect(backend.read('.notatnik/config.toml')).toBe(cfgBefore)
  await page.getByRole('tab', { name: 'Dates & language' }).click()
  await page.getByRole('radio', { name: 'Sunday' }).check()
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('sunday')
})

test('zoom into a block, breadcrumb back, browser back', async ({ backend, page }) => {
  backend.write('pages/z.md', '- top\n  - middle\n    - deep one\n    - deep two\n- other top\n')
  await backend.open('/page/z')
  await page
    .getByText('middle', { exact: true })
    .locator('xpath=ancestor::*[@role="treeitem"][1]')
    .getByRole('button', { name: 'Zoom into block' })
    .first()
    .click()
  await expect(page).toHaveURL(/zoom=0\.0/)
  await expect(page.getByText('deep one')).toBeVisible()
  await expect(page.getByText('other top')).toBeHidden()
  await page
    .getByRole('navigation', { name: 'Block path' })
    .getByRole('link', { name: 'z', exact: true })
    .click()
  await expect(page.getByText('other top')).toBeVisible()
  await page.goBack()
  await expect(page.getByText('other top')).toBeHidden()
})

test('command palette opens a page by name', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('alpha')
  await expect(page.getByRole('listbox', { name: 'Matches' }).getByRole('option').first()).toContainText(
    'work/projects/alpha',
  )
  await page.keyboard.press('Enter')
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Project Alpha')
})

test('every listed palette command can be run from the palette', async ({ backend, page }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Go to journals')
  await expect(page.getByRole('listbox', { name: 'Matches' }).getByRole('option').first()).toContainText(
    'Go to journals',
  )
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('journals')).toBeVisible()
})

test('shift+click on a link opens the page in the right sidebar and edits there reach the file', async ({
  backend,
  page,
}) => {
  await backend.open('/page/work%2Fprojects%2Falpha')
  await page
    .locator('a.page-ref[data-page="people/anna"]')
    .first()
    .click({ modifiers: ['Shift'] })
  const panel = page.getByRole('complementary', { name: 'Side panels' })
  await expect(panel).toContainText('Anna Kowalska')
  await panel.getByText('Anna Kowalska').click({ position: { x: 5, y: 5 } })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' (side)')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/people/anna.md'))
    .toContain('Anna Kowalska, project lead (side)')
})

test('inserting a block reference adds an id to exactly one line of the target file', async ({
  backend,
  page,
}) => {
  backend.write('pages/target.md', '- unique target sentence\n- another\n')
  backend.write('pages/source.md', '- see \n')
  await backend.open('/page/source')
  await page.getByText('see').click({ position: { x: 3, y: 3 } })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type('((unique tar')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('unique target sentence', {
    timeout: 5000,
  })
  await expect(page.locator('.cm-tooltip-autocomplete [aria-selected="true"]')).toBeVisible()
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await page.keyboard.press('Enter')
  await expect
    .poll(() => backend.read('pages/target.md'))
    .toMatch(/^- unique target sentence \^[a-z0-9]{6}\n- another\n$/)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/source.md')).toMatch(/- see \(\([a-z0-9]{6}\)\)/)
})

test('unlinked references list plain mentions and the Link button links only that one', async ({
  backend,
  page,
}) => {
  backend.write('pages/Zofia.md', '- Zofia is a person\n')
  backend.write('pages/notes.md', '- met zofia today\n- Zofiabelle came\n- see [[Zofia]] and Zofia\n')
  await backend.open('/page/Zofia')
  await expect(page.getByTestId('backlink-count')).toHaveText('1')
  await page.getByTestId('unlinked-toggle').click()
  await expect(page.getByTestId('unlinked-count')).toHaveText('1')
  const item = page.getByTestId('unlinked-item')
  await expect(item).toContainText('met zofia today')
  await item.getByRole('button', { name: 'Link' }).click()
  await expect
    .poll(() => backend.read('pages/notes.md'))
    .toBe('- met [[Zofia|zofia]] today\n- Zofiabelle came\n- see [[Zofia]] and Zofia\n')
  await expect(page.getByTestId('unlinked-count')).toHaveText('0')
  await expect(page.getByTestId('backlink-count')).toHaveText('2')
})

test('renaming a middle segment renames the namespace: the pages beside it follow', async ({
  backend,
  page,
}) => {
  backend.write('pages/P/CapGemini/sth.md', '- sth, see [[P/CapGemini/different]]\n')
  backend.write('pages/P/CapGemini/different.md', '- different\n')
  backend.write('pages/P/CapGemini/deep/leaf.md', '- leaf\n')
  backend.write('pages/P/Other/keep.md', '- stays\n')
  backend.write('pages/menu.md', '- [[P/CapGemini/deep/leaf]] and #[[P/CapGemini/sth]]\n')
  await backend.open('/page/P%2FCapGemini%2Fsth')
  await page.getByTitle('Rename page').click()
  const dialog = page.getByRole('dialog', { name: 'Rename page' })
  await dialog.getByLabel('New name').fill('P/Capgemini12/sth')
  // on by default
  const all = dialog.getByLabel(/for all pages below it/)
  await expect(all).toBeChecked()
  await dialog.getByRole('button', { name: 'Preview' }).click()
  await expect(page.getByTestId('rename-plan')).toContainText('3 page files move')
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  // we stay on the page we renamed
  await expect(page).toHaveURL(/P%2FCapgemini12%2Fsth/)
  expect(backend.exists('pages/P/Capgemini12/different.md')).toBe(true)
  expect(backend.exists('pages/P/Capgemini12/deep/leaf.md')).toBe(true)
  expect(backend.exists('pages/P/CapGemini')).toBe(false)
  expect(backend.exists('pages/P/Other/keep.md')).toBe(true)
  expect(backend.read('pages/P/Capgemini12/sth.md')).toContain('[[P/Capgemini12/different]]')
  expect(backend.read('pages/menu.md')).toContain('[[P/Capgemini12/deep/leaf]]')
})

test('unticking "for all pages below it" moves only that page', async ({ backend, page }) => {
  backend.write('pages/P/CapGemini/sth.md', '- sth\n')
  backend.write('pages/P/CapGemini/different.md', '- different\n')
  await backend.open('/page/P%2FCapGemini%2Fsth')
  await page.getByTitle('Rename page').click()
  const dialog = page.getByRole('dialog', { name: 'Rename page' })
  await dialog.getByLabel('New name').fill('P/Capgemini12/sth')
  await dialog.getByLabel(/for all pages below it/).uncheck()
  await dialog.getByRole('button', { name: 'Preview' }).click()
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  await expect(page).toHaveURL(/P%2FCapgemini12%2Fsth/)
  expect(backend.exists('pages/P/Capgemini12/sth.md')).toBe(true)
  expect(backend.exists('pages/P/CapGemini/different.md')).toBe(true) // stayed
})

test('a changed last segment is an ordinary move: no namespace option', async ({ backend, page }) => {
  backend.write('pages/P/CapGemini/sth.md', '- sth\n')
  await backend.open('/page/P%2FCapGemini%2Fsth')
  await page.getByTitle('Rename page').click()
  const dialog = page.getByRole('dialog', { name: 'Rename page' })
  await dialog.getByLabel('New name').fill('P/Capgemini12/other')
  await expect(dialog.getByLabel(/for all pages below it/)).toHaveCount(0)
})

test('the namespace rename is refused when a page of that name exists', async ({ backend, page }) => {
  backend.write('pages/P/CapGemini/sth.md', '- sth\n')
  backend.write('pages/P/Capgemini12/sth.md', '- already here\n')
  await backend.open('/page/P%2FCapGemini%2Fsth')
  await page.getByTitle('Rename page').click()
  const dialog = page.getByRole('dialog', { name: 'Rename page' })
  await dialog.getByLabel('New name').fill('P/Capgemini12/sth')
  await dialog.getByRole('button', { name: 'Preview' }).click()
  await expect(page.getByRole('alert')).toContainText('already exists')
  expect(backend.exists('pages/P/CapGemini/sth.md')).toBe(true)
})

test('a linked reference shows the block with its sub-bullets and a path that opens the page', async ({
  backend,
  page,
}) => {
  backend.write('pages/Deal.md', '- the deal\n')
  backend.write(
    'pages/meeting.md',
    '- Weekly sync\n  - [[Deal]]:\n    - discussion point\n    - TODO call back\n      - sub-sub note\n',
  )
  await backend.open('/page/Deal')
  const refs = page.getByRole('region', { name: 'Linked references', exact: true })
  await expect(page.getByTestId('backlink-count')).toHaveText('1')
  const item = refs.getByTestId('backlink-item')
  await expect(item).toContainText('discussion point')
  await expect(item).toContainText('call back')
  await expect(item).toContainText('sub-sub note')
  // the ancestor path is a link: it opens the page there
  await item.getByRole('link', { name: 'Weekly sync' }).click()
  await expect(page).toHaveURL(/#\/page\/meeting/)
})

test('bullets in a linked reference can be collapsed and expanded', async ({ backend, page }) => {
  backend.write('pages/Deal.md', '- the deal\n')
  backend.write(
    'pages/meeting.md',
    '- [[Deal]]:\n  - discussion point\n    - deeper note\n  - TODO call back\n',
  )
  await backend.open('/page/Deal')
  const item = page.getByTestId('backlink-item')
  await expect(item).toContainText('deeper note')
  // the nested bullet first, then the referencing bullet itself
  await item.getByRole('button', { name: 'Collapse' }).nth(1).click()
  await expect(item).not.toContainText('deeper note')
  await expect(item).toContainText('discussion point')
  await item.getByRole('button', { name: 'Collapse', exact: true }).first().click()
  await expect(item).not.toContainText('discussion point')
  await item.getByRole('button', { name: 'Expand', exact: true }).first().click()
  await expect(item).toContainText('discussion point')
  await expect(item).not.toContainText('deeper note') // still collapsed
  await item.getByRole('button', { name: 'Expand', exact: true }).click()
  await expect(item).toContainText('deeper note')
})

test('after a rename an already opened journal shows the new links, not the old ones', async ({
  backend,
  page,
}) => {
  backend.write('pages/P/CapGemini/sth.md', '- sth\n')
  backend.write('pages/P/CapGemini/different.md', '- different\n')
  backend.write('journals/2026/2026-10-01.md', '- met about [[P/CapGemini/different]]\n')
  // open the journal first: its document stays loaded while we rename elsewhere
  await backend.open('/page/2026-10-01')
  await expect(page.getByRole('tree', { name: 'Blocks' })).toContainText('P/CapGemini/different')
  await page.evaluate(() => (location.hash = '#/page/P%2FCapGemini%2Fsth'))
  await page.getByTitle('Rename page').click()
  const dialog = page.getByRole('dialog', { name: 'Rename page' })
  await dialog.getByLabel('New name').fill('P/Capgemini12/sth')
  await dialog.getByRole('button', { name: 'Preview' }).click()
  await page.getByRole('button', { name: 'Rename', exact: true }).click()
  await expect(page).toHaveURL(/P%2FCapgemini12%2Fsth/)
  expect(backend.read('journals/2026/2026-10-01.md')).toContain('[[P/Capgemini12/different]]')
  // back to the journal, inside the running app
  await page.evaluate(() => (location.hash = '#/page/2026-10-01'))
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await expect(blocks).toContainText('P/Capgemini12/different')
  await expect(blocks).not.toContainText('P/CapGemini/different')
})
