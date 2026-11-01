import http from 'node:http'
import type { Page } from '@playwright/test'
import { expect, test } from './fixtures'

// ED-27: smart paste of links.

/** Pastes `plain` (and `html`) into the block being edited, as a browser paste would. */
async function paste(page: Page, plain: string, html = '') {
  await page.evaluate(
    ([p, h]) => {
      const dt = new DataTransfer()
      dt.setData('text/plain', p)
      if (h) dt.setData('text/html', h)
      document
        .querySelector('[data-testid="block-editor"]')!
        .dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: dt }))
    },
    [plain, html],
  )
}

async function edit(page: Page, backend: { open: (r: string) => Promise<void> }, text: string) {
  await backend.open('/page/Links')
  await page.getByRole('tree', { name: 'Blocks' }).getByText(text, { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
}

test('an address over selected text becomes a link with that text, without tracking parameters', async ({
  backend,
  page,
}) => {
  backend.write('pages/Links.md', '- the report\n')
  await edit(page, backend, 'the report')
  await page.keyboard.press('Control+Shift+a') // the text of the block
  await paste(page, 'https://x.org/r?utm_source=news&id=3')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/Links.md')).toBe('- [the report](https://x.org/r?id=3)\n')
})

test('a link copied from a web page keeps its text; a Nextcloud file gets the stable address', async ({
  backend,
  page,
}) => {
  backend.write('pages/Links.md', '- start\n')
  await edit(page, backend, 'start')
  await page.keyboard.press('End')
  await paste(page, 'The report', '<meta charset="utf-8"><a href="https://x.org/p?utm_x=1">The report</a>')
  await page.keyboard.type(' ')
  await paste(page, 'https://nc.example.eu/apps/files/?dir=/Docs&fileid=4711&openfile=true')
  await page.keyboard.type(' ')
  await paste(page, 'notatnik://page/Plan')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/Links.md'))
    .toBe('- start[The report](https://x.org/p) [Nextcloud file](https://nc.example.eu/f/4711) [[Plan]]\n')
})

test('a bare address and text are pasted as they are; nothing changes inside a link', async ({
  backend,
  page,
}) => {
  await backend.call('set_settings', { paste_titles: false }) // on by default: it would ask x.org for a title
  backend.write('pages/Links.md', '- a\n')
  await edit(page, backend, 'a')
  await page.keyboard.press('End')
  await paste(page, ' https://x.org/clean')
  await paste(page, ' and [see](')
  await paste(page, 'https://x.org/t?utm_source=a')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/Links.md'))
    .toBe('- a https://x.org/clean and [see](https://x.org/t?utm_source=a\n')
})

test('with the setting on, a lone address gets the page title; one undo gives the address back', async ({
  backend,
  page,
}) => {
  const server = http.createServer((_, res) => {
    res.writeHead(200, { 'Content-Type': 'text/html' })
    res.end('<html><head><title>Hello &amp; welcome</title></head></html>')
  })
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r))
  try {
    const url = `http://127.0.0.1:${(server.address() as { port: number }).port}/p`
    backend.write('pages/Links.md', '- start\n')
    // off: the address stays as it is, and nothing was looked up
    await backend.call('set_settings', { paste_titles: false })
    await edit(page, backend, 'start')
    await page.keyboard.press('End')
    await paste(page, ` ${url}`)
    await page.waitForTimeout(300)
    await page.keyboard.press('Escape')
    await expect.poll(() => backend.read('pages/Links.md')).toBe(`- start ${url}\n`)

    await backend.call('set_settings', { paste_titles: true })
    await page.reload() // the settings are read when the app starts
    backend.write('pages/Links.md', '- start\n')
    await edit(page, backend, 'start')
    await page.keyboard.press('End')
    await page.keyboard.type(' ')
    await paste(page, url)
    // the title arrives a moment later; leaving the block before that keeps the address
    await expect(page.getByTestId('block-editor')).toContainText('Hello & welcome')
    // one undo gives the plain address back, in its own step
    await page.keyboard.press('Control+z')
    await expect(page.getByTestId('block-editor')).toHaveText(`start ${url}`)
    await page.keyboard.press('Escape')
    await expect.poll(() => backend.read('pages/Links.md')).toBe(`- start ${url}\n`)
  } finally {
    server.close()
  }
})

test('the setting is in Settings → Appearance and is on by default', async ({ backend, page }) => {
  await backend.open('/settings')
  const sw = page.getByRole('switch', { name: 'Look up titles of pasted links' })
  await expect(sw).toBeChecked()
  await sw.click()
  await expect(page.getByText('✓ Saved', { exact: true })).toBeVisible()
  const s = await backend.call<{ paste_titles: boolean }>('get_settings', {})
  expect(s.paste_titles).toBe(false)
})

test('a GitHub issue gets its short name and a file path a link named after the file, with no request', async ({
  backend,
  page,
}) => {
  backend.write('pages/Links.md', '- start\n')
  await edit(page, backend, 'start')
  await page.keyboard.press('End')
  await page.keyboard.type(' ')
  await paste(page, 'https://github.com/o/r/issues/12?utm_source=x')
  await page.keyboard.type(' ')
  await paste(page, '/home/me/My Docs/a.pdf')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/Links.md'))
    .toBe('- start [o/r#12](https://github.com/o/r/issues/12) [a.pdf](file:///home/me/My%20Docs/a.pdf)\n')
})

test('several addresses, one per line, become one block each with its title', async ({ backend, page }) => {
  const server = http.createServer((req, res) => {
    res.writeHead(200, { 'Content-Type': 'text/html' })
    res.end(`<html><head><title>Page ${req.url}</title></head></html>`)
  })
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r))
  try {
    const base = `http://127.0.0.1:${(server.address() as { port: number }).port}`
    backend.write('pages/Links.md', '- start\n')
    await edit(page, backend, 'start')
    await page.keyboard.press('End')
    await paste(page, `${base}/one\n${base}/two?utm_source=x\nhttps://github.com/o/r/pull/3`)
    await page.keyboard.press('Escape')
    await expect
      .poll(() => backend.read('pages/Links.md'))
      .toBe(
        `- start\n- [Page /one](${base}/one)\n- [Page /two](${base}/two)\n- [o/r#3](https://github.com/o/r/pull/3)\n`,
      )
  } finally {
    server.close()
  }
})
