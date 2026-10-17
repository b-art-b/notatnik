import { expect, test } from './fixtures'

// Pages written by other Markdown editors (Nextcloud Text, Obsidian): headings and tables between bullets, checkbox
// tasks, attachments in a `.attachments.N` folder next to the page.
const PAGE = `# Notes

* [ ] ship it
* plain bullet

## Heading

| a | b |
|---|---|
| 1 | 2 |

* last bullet

![Makefile](.attachments.1/Makefile)

![pic](.attachments.1/pic.svg)
`

test('a checkbox is a task that stays a checkbox when it is ticked', async ({ backend, page }) => {
  backend.write('pages/Interop.md', PAGE)
  await backend.open('/page/Interop')
  const tree = page.getByRole('tree', { name: 'Blocks' })
  const box = tree.getByRole('button', { name: '☐' })
  await expect(box).toBeVisible()
  await expect(box).toHaveAttribute('data-task', 'TODO')
  await box.click()
  await expect(tree.getByRole('button', { name: '☑' })).toBeVisible()
  await expect
    .poll(() => backend.read('pages/Interop.md'))
    .toBe(PAGE.replace('* [ ] ship it', '* [x] ship it'))
  // and back
  await tree.getByRole('button', { name: '☑' }).click()
  await expect.poll(() => backend.read('pages/Interop.md')).toBe(PAGE)
})

test('text between bullets has no bullet, takes Enter as a new line and is written back as typed', async ({
  backend,
  page,
}) => {
  backend.write('pages/Interop.md', PAGE)
  await backend.open('/page/Interop')
  const tree = page.getByRole('tree', { name: 'Blocks' })
  const heading = tree.getByRole('treeitem').filter({ has: page.getByRole('heading', { name: 'Heading' }) })
  await expect(heading).toHaveCount(1)
  // no bullet to zoom into
  await expect(heading.getByRole('button', { name: 'Zoom into block' })).toHaveCount(0)
  await expect(
    tree
      .getByRole('treeitem')
      .filter({ hasText: 'plain bullet' })
      .getByRole('button', { name: 'Zoom into block' }),
  ).toHaveCount(1)

  await heading.getByText('Heading').click()
  const editor = page.getByTestId('block-editor')
  await expect(editor).toBeFocused()
  await expect(editor).toContainText('## Heading')
  await page.keyboard.press('Control+End')
  await page.keyboard.press('Enter') // a new line, not a new block
  await page.keyboard.type('extra line')
  await page.keyboard.press('Tab') // does nothing: text cannot be nested
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/Interop.md'))
    .toBe(PAGE.replace('| 1 | 2 |\n', '| 1 | 2 |\nextra line\n'))
  // the bullets around it are untouched
  expect(backend.read('pages/Interop.md')).toContain('* plain bullet\n\n## Heading')
  expect(backend.read('pages/Interop.md')).toContain('extra line\n\n* last bullet')
})

test('attachments next to the page: a file is a link, a picture is shown', async ({ backend, page }) => {
  backend.write('pages/Interop.md', PAGE)
  backend.write('pages/.attachments.1/Makefile', 'all:\n\techo hi\n')
  backend.write(
    'pages/.attachments.1/pic.svg',
    '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>',
  )
  await backend.open('/page/Interop')
  const link = page.locator('a.external', { hasText: 'Makefile' })
  await expect(link).toBeVisible()
  await expect(link).toHaveAttribute('href', 'pages/.attachments.1/Makefile')
  const img = page.locator('img.inline-image[alt="pic"]')
  await expect(img).toBeVisible()
  const src = await img.getAttribute('src')
  const res = await page.request.get(src!)
  expect(res.status()).toBe(200)
  expect(res.headers()['content-type']).toContain('image/svg+xml')
  // the settings file of the graph is not served this way
  const base = src!.split('/asset?')[0]
  expect(
    (await page.request.get(`${base}/asset?path=${encodeURIComponent('.notatnik/config.toml')}`)).status(),
  ).not.toBe(200)
})
