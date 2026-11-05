import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from './fixtures'
import { article, pageServer, type PageServer } from './web-fixture'

// WA-1, WA-2, WA-6, WA-7: `/clip` and the palette command clip a web page into a book below assets/web and link it.
// The dev server runs with --allow-local-fetch (the page server is on this computer); `clip-refused.spec.ts` runs
// without it.
test.use({ localFetch: true })

let web: PageServer
test.beforeEach(async () => {
  web = await pageServer()
})
test.afterEach(async () => {
  await web.close()
})

type Backend = Parameters<Parameters<typeof test>[2]>[0]['backend']

/** Opens a page with one block, types `/clip` at the end of it and opens the dialog. */
async function slashClip(page: Page, backend: Backend, name = 'notes') {
  backend.write(`pages/${name}.md`, '- start\n')
  await backend.open(`/page/${name}`)
  await page.getByRole('tree', { name: 'Blocks' }).getByText('start', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /clip')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Clip a web page…', { timeout: 5000 })
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('clip-dialog')).toBeVisible()
}

const books = (backend: Backend) =>
  fs.existsSync(path.join(backend.dir, 'assets/web'))
    ? fs.readdirSync(path.join(backend.dir, 'assets/web'))
    : []

test('/clip: the address is clipped, the link and the source go where the command was, the link opens the book', async ({
  backend,
  page,
}) => {
  await slashClip(page, backend)
  // a labelled dialog that took the focus
  await expect(page.getByRole('dialog', { name: 'Clip a web page' })).toBeVisible()
  await expect(page.getByTestId('clip-address')).toBeFocused()
  await page.getByTestId('clip-address').fill(`${web.base}/article`)
  await page.getByTestId('clip-go').click()
  await expect(page.getByTestId('clip-dialog')).toHaveCount(0)
  // the block is still being edited, with the link in it
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/notes.md'))
    .toMatch(
      /^- start \[The Lantern[^\]]*\]\(\.\.\/assets\/web\/the-lantern[a-z-]*-[0-9a-f]{8}\.epub\)\n {2}source:: http:\/\/127\.0\.0\.1:\d+\/article\n$/,
    )
  expect(books(backend)).toHaveLength(1)
  // the link opens the reader on the clip
  await page
    .getByRole('tree', { name: 'Blocks' })
    .getByRole('link', { name: /The Lantern/ })
    .click()
  await expect(page.getByTestId('epub-reader')).toBeVisible()
  await expect
    .poll(async () =>
      (
        await Promise.all(
          page
            .frames()
            .filter((f) => f !== page.mainFrame())
            .map((f) =>
              f
                .locator('body')
                .innerText()
                .catch(() => ''),
            ),
        )
      ).join(),
    )
    .toContain('harbour town woke slowly')
})

test('the clip is one undo step', async ({ backend, page }) => {
  await slashClip(page, backend)
  await page.getByTestId('clip-address').fill(`${web.base}/article`)
  await page.getByTestId('clip-address').press('Enter')
  await expect(page.getByTestId('clip-dialog')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('block-editor')).toHaveCount(0)
  await expect.poll(() => backend.read('pages/notes.md')).toContain('source:: ')
  await page.waitForTimeout(300) // let the UI settle: a human can't press keys this fast
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/notes.md')).toBe('- start\n')
})

test('the palette command adds a block with the link to the open page', async ({ backend, page }) => {
  backend.write('pages/notes.md', '- first\n')
  await backend.open('/page/notes')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('clip a web')
  await page.keyboard.press('Enter')
  await page.getByTestId('clip-address').fill(`${web.base}/moved`)
  await page.getByTestId('clip-go').click()
  await expect(page.getByTestId('clip-dialog')).toHaveCount(0)
  await expect
    .poll(() => backend.read('pages/notes.md'))
    .toMatch(
      /^- first\n- \[The Lantern[^\]]*\]\(\.\.\/assets\/web\/[^)]+\.epub\)\n {2}source:: http:\/\/127\.0\.0\.1:\d+\/article\n$/,
    )
})

test('the dialog shows progress and Cancel stops the clip without writing anything', async ({
  backend,
  page,
}) => {
  await slashClip(page, backend)
  await page.getByTestId('clip-address').fill(`${web.base}/slow`)
  await page.getByTestId('clip-go').click()
  await expect(page.getByTestId('clip-progress')).toContainText('Fetching the page')
  await expect.poll(() => web.slowHits()).toBe(1)
  await page.getByTestId('clip-cancel').click()
  // back to the form, no error, the address is still there
  await expect(page.getByTestId('clip-go')).toBeVisible()
  await expect(page.getByTestId('clip-error')).toHaveCount(0)
  await expect(page.getByTestId('clip-address')).toHaveValue(`${web.base}/slow`)
  expect(books(backend)).toEqual([])
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('clip-dialog')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/notes.md')).toBe('- start\n')
})

for (const [what, path, words] of [
  ['a missing page', '/nothing-here', /404|not found/i],
  ['an address that is not a web page', '/data.json', /web page|html/i],
  ['an unreachable site', null, /./],
] as const) {
  test(`${what} shows its reason and writes nothing`, async ({ backend, page }) => {
    await slashClip(page, backend)
    await page.getByTestId('clip-address').fill(path === null ? 'http://127.0.0.1:1/x' : `${web.base}${path}`)
    await page.getByTestId('clip-go').click()
    await expect(page.getByTestId('clip-error')).toHaveText(words)
    // the dialog stays so the address can be corrected
    await expect(page.getByTestId('clip-go')).toBeEnabled()
    expect(books(backend)).toEqual([])
    await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
    await expect.poll(() => backend.read('pages/notes.md')).toBe('- start\n')
  })
}

test('something that is not a web address is not sent', async ({ backend, page }) => {
  await slashClip(page, backend)
  await page.getByTestId('clip-address').fill('not an address')
  await page.getByTestId('clip-go').click()
  await expect(page.getByTestId('clip-error')).toContainText('http')
  expect(books(backend)).toEqual([])
})

test('the same address again asks first, then replaces the saved copy', async ({ backend, page }) => {
  await slashClip(page, backend)
  await page.getByTestId('clip-address').fill(`${web.base}/article`)
  await page.getByTestId('clip-go').click()
  await expect(page.getByTestId('clip-dialog')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/notes.md')).toContain('source:: ')
  const [file] = books(backend)
  const before = fs.readFileSync(path.join(backend.dir, 'assets/web', file))

  // the page changed: a paragraph above the old text
  web.articleHtml = article('The Lantern', '<p>A new paragraph about the ferry.</p>')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('start').click()
  await page.keyboard.press('End')
  await page.keyboard.type(' /clip')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Clip a web page…')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await page.getByTestId('clip-address').fill(`${web.base}/article`)
  await page.getByTestId('clip-go').click()
  await expect(page.getByTestId('clip-again')).toContainText('highlights stay')
  expect(fs.readFileSync(path.join(backend.dir, 'assets/web', file))).toEqual(before)
  await page.getByTestId('clip-go').click()
  await expect(page.getByTestId('clip-dialog')).toHaveCount(0)
  // the same file name, new bytes
  expect(books(backend)).toEqual([file])
  expect(fs.readFileSync(path.join(backend.dir, 'assets/web', file))).not.toEqual(before)
})

test('a web address on the clipboard is put in the field', async ({
  backend,
  page,
  context,
  browserName,
}) => {
  test.skip(browserName !== 'chromium', 'clipboard permissions are a Chromium feature of the test driver')
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  backend.write('pages/notes.md', '- start\n')
  await backend.open('/page/notes')
  await page.evaluate((t) => navigator.clipboard.writeText(t), `${web.base}/article`)
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('clip a web')
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('clip-address')).toHaveValue(`${web.base}/article`)
})

test('text on the clipboard that is no address is left out', async ({
  backend,
  page,
  context,
  browserName,
}) => {
  test.skip(browserName !== 'chromium', 'clipboard permissions are a Chromium feature of the test driver')
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  backend.write('pages/notes.md', '- start\n')
  await backend.open('/page/notes')
  await page.evaluate(() => navigator.clipboard.writeText('just some words'))
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('clip a web')
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('clip-address')).toBeFocused()
  await expect(page.getByTestId('clip-address')).toHaveValue('')
})

test('Escape closes the dialog without clipping, with the keyboard only', async ({ backend, page }) => {
  await slashClip(page, backend)
  await page.keyboard.type(`${web.base}/article`)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('clip-dialog')).toHaveCount(0)
  expect(books(backend)).toEqual([])
  // the block is still being edited and nothing was written
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/notes.md')).toBe('- start\n')
})
