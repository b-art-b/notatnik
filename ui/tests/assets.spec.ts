import { expect, test } from './fixtures'

// ST-8: a screenshot pasted into a block (or an image file dropped on it) is stored below assets/ and linked.
const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='

/** Sends a paste or drop event with a PNG file to the open editor, as the browser does for a clipboard image. */
async function sendImage(page: import('@playwright/test').Page, kind: 'paste' | 'drop', name: string) {
  await page.evaluate(
    ([kind, name, b64]) => {
      const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
      const dt = new DataTransfer()
      dt.items.add(new File([bytes], name, { type: 'image/png' }))
      const target = document.querySelector('[data-testid="block-editor"]')!
      const init = { bubbles: true, cancelable: true }
      target.dispatchEvent(
        kind === 'paste'
          ? new ClipboardEvent('paste', { ...init, clipboardData: dt })
          : new DragEvent('drop', { ...init, dataTransfer: dt }),
      )
    },
    [kind, name, PNG] as const,
  )
}

test('a pasted screenshot becomes a file in assets/ and a link that renders', async ({ backend, page }) => {
  await backend.open('/page/inbox')
  await page.getByText('Call the dentist').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await sendImage(page, 'paste', 'image.png')
  const editor = page.getByTestId('block-editor')
  await expect(editor).toContainText('![](../assets/')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/inbox.md'))
    .toMatch(/^ *- Call the dentist #health !\[\]\(\.\.\/assets\/\d{4}\/\d{2}\/pasted-[0-9a-f]{8}\.png\)$/m)
  const rel = /\(\.\.\/(assets\/[^)]+)\)/.exec(backend.read('pages/inbox.md'))![1]
  expect(backend.exists(rel)).toBe(true)

  // the picture is shown in the block, not just its source
  const img = page.getByRole('tree', { name: 'Blocks' }).locator('img.inline-image').first()
  await expect(img).toBeVisible()
  await expect.poll(() => img.evaluate((i: HTMLImageElement) => i.naturalWidth)).toBe(1)

  // the same picture again: still one file
  await page.getByText('Call the dentist').click()
  await sendImage(page, 'paste', 'image.png')
  await expect(page.getByTestId('block-editor')).toContainText('png) ![](')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/inbox.md')).toMatch(/png\) !\[\]\(\.\.\/assets\//)
  const links = backend.read('pages/inbox.md').match(/assets\/[^)]+\.png/g)
  expect(links).toEqual([rel, rel])
})

test('a dropped image file keeps its name in the stored file', async ({ backend, page }) => {
  await backend.open('/page/inbox')
  await page.getByText('Call the dentist').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await sendImage(page, 'drop', 'floor plan.png')
  await expect(page.getByTestId('block-editor')).toContainText('floor-plan-')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/inbox.md'))
    .toMatch(/\]\(\.\.\/assets\/\d{4}\/\d{2}\/floor-plan-[0-9a-f]{8}\.png\)/)
})

test('unused images are listed in Settings and can be deleted; a linked one stays', async ({
  backend,
  page,
}) => {
  await backend.open('/page/inbox')
  const save = async (name: string) =>
    (await backend.call<{ path: string }>('save_asset', { page: 'inbox', data: PNG, name })).path
  const kept = await save('kept.png')
  // the same bytes again would be the same file, so make a second picture by changing the last chunk
  const other = Buffer.from(PNG, 'base64')
  other[other.length - 20] ^= 1
  const lost = (
    await backend.call<{ path: string }>('save_asset', {
      page: 'inbox',
      data: other.toString('base64'),
      name: 'lost.png',
    })
  ).path
  backend.write('pages/pics.md', `- ![](../${kept})\n`)

  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Graph' }).click()
  await page.getByTestId('unused-look').click()
  await expect(page.getByTestId('unused-count')).toContainText('1 unused')
  await expect(page.getByTestId('unused-list')).toContainText(lost)
  await expect(page.getByTestId('unused-list')).not.toContainText(kept)
  await page.getByTestId('unused-trash').click()
  await expect(page.getByTestId('unused-none')).toBeVisible()
  expect(backend.exists(lost)).toBe(false)
  expect(backend.exists(kept)).toBe(true)
})
