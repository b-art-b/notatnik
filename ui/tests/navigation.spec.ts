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
  await page.getByRole('button', { name: 'Collapse' }).first().click()
  await expect(page.getByText('Renew passport')).toBeHidden()
  await page.waitForTimeout(300)
  await page.reload()
  await expect(page.getByText('Things to sort out')).toBeVisible()
  await expect(page.getByText('Renew passport')).toBeHidden()
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
