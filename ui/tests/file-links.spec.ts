import { spawn, type ChildProcess } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test as base } from './fixtures'

// A graph kept in a Nextcloud/WebDAV folder (SY-1..SY-8), against the in-memory WebDAV server of the sync tests.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')

interface Dav {
  server: string
  user: string
  password: string
}

const test = base.extend<{ dav: Dav }>({
  // Playwright reads the fixtures a fixture needs from the destructuring of its first argument: none here
  // eslint-disable-next-line no-empty-pattern
  dav: async ({}, use) => {
    const proc: ChildProcess = spawn(path.join(root, 'target/debug/notatnik-fakedav'), [], {
      stdio: ['ignore', 'pipe', 'inherit'],
    })
    const line = await new Promise<string>((resolve, reject) => {
      let buf = ''
      proc.stdout!.on('data', (d) => {
        buf += d
        if (buf.includes('\n')) resolve(buf.split('\n')[0])
      })
      proc.on('exit', (c) => reject(new Error(`fakedav exited early (${c})`)))
    })
    const [, url, user, ...pw] = line.split(' ')
    await use({ server: url.replace(/\/dav$/, ''), user, password: pw.join(' ') })
    proc.kill()
  },
})

/** Puts a file on the fake server (the folders above it first). */
async function put(dav: Dav, file: string, body = 'x') {
  const auth = 'Basic ' + Buffer.from(`${dav.user}:${dav.password}`).toString('base64')
  const base = `${dav.server}/remote.php/dav/files/${dav.user}`
  const parts = file.split('/')
  for (let i = 1; i < parts.length; i++) {
    await fetch(`${base}/${parts.slice(0, i).map(encodeURIComponent).join('/')}`, {
      method: 'MKCOL',
      headers: { Authorization: auth },
    })
  }
  await fetch(`${base}/${parts.map(encodeURIComponent).join('/')}`, {
    method: 'PUT',
    headers: { Authorization: auth },
    body,
  })
}

async function typeFileCommand(
  page: import('@playwright/test').Page,
  backend: { open(p: string): Promise<void> },
) {
  await backend.open('/page/t')
  await page.getByRole('tree', { name: 'Blocks' }).getByText('see the manual').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /file')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Link a file/)
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('file-picker')).toBeVisible()
}

test('/file adds a server, browses it and links a file by its Nextcloud number', async ({
  backend,
  page,
  dav,
}) => {
  await put(dav, 'Docs/Manual v2.pdf')
  await put(dav, 'Docs/Old/notes.txt')
  backend.write('pages/t.md', '- see the manual\n')
  await typeFileCommand(page, backend)
  const picker = page.getByTestId('file-picker')

  // no server yet: add one (a wrong password is refused, nothing is kept)
  await picker.getByTestId('file-add-server').click()
  await picker.getByLabel('Name', { exact: true }).fill('Cloud')
  await picker.getByLabel('Server', { exact: true }).fill(dav.server)
  await picker.getByLabel('User name').fill(dav.user)
  await picker.getByLabel(/^Password/).fill('nope')
  await picker.getByRole('button', { name: 'Save' }).click()
  await expect(picker.getByRole('alert')).toContainText('refused the login')
  await picker.getByLabel(/^Password/).fill(dav.password)
  await picker.getByRole('button', { name: 'Save' }).click()

  // the server opens; click through to the file
  await picker.getByRole('button', { name: /Docs/ }).click()
  await expect(picker.getByRole('navigation', { name: 'Where you are' })).toContainText('Docs')
  await expect(picker.getByTestId('file-entry')).toHaveCount(1)
  await picker.getByLabel('Filter by name').fill('zzz')
  await expect(picker.getByText('Nothing matches.')).toBeVisible()
  await picker.getByLabel('Filter by name').fill('')
  await picker.getByTestId('file-entry').click()

  await expect(picker).toBeHidden()
  const editor = page.getByTestId('block-editor')
  await expect(editor).toBeFocused()
  await expect(editor).toContainText(`[Manual v2.pdf](${dav.server}/f/`)
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toMatch(
      new RegExp(
        `^- see the manual \\[Manual v2\\.pdf\\]\\(${dav.server.replace(/[.]/g, '\\.')}/f/\\d+\\)\n$`,
      ),
    )
  // shown as a link
  await expect(page.locator('a.external', { hasText: 'Manual v2.pdf' })).toBeVisible()

  // the next time the server is in the list, and Escape steps back before it closes the picker
  await page.getByText('see the manual').click()
  await page.keyboard.press('End')
  await page.keyboard.type(' /file')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Link a file/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await picker.getByTestId('file-source').click()
  await picker.getByRole('button', { name: /Docs/ }).click()
  await page.keyboard.press('Escape')
  await expect(picker.getByRole('navigation', { name: 'Where you are' })).not.toContainText('Docs')
  await page.keyboard.press('Escape')
  await expect(picker.getByRole('heading', { name: /Link a file from/ })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(picker).toBeHidden()
})

test('/file copies a chosen file into the graph and links it; an address is linked as typed', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- see the manual\n')
  await typeFileCommand(page, backend)
  const picker = page.getByTestId('file-picker')
  // a browser has no way to link a file in place, only to copy it
  await picker.getByTestId('file-computer').click()
  await picker.getByTestId('file-upload').setInputFiles({
    name: 'Price list.pdf',
    mimeType: 'application/pdf',
    buffer: Buffer.from('%PDF-1.4 test'),
  })
  await expect(picker).toBeHidden()
  await expect(page.getByTestId('block-editor')).toContainText('[Price list.pdf](../assets/')
  await page.keyboard.press('Escape')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toMatch(
      /^- see the manual \[Price list\.pdf\]\(\.\.\/assets\/\d{4}\/\d{2}\/Price-list-[0-9a-f]{8}\.pdf\)\n$/,
    )
  const rel = /\(\.\.\/(assets\/[^)]+)\)/.exec(backend.read('pages/t.md'))![1]
  expect(backend.exists(rel)).toBe(true)
  await expect(page.locator('a.external', { hasText: 'Price list.pdf' })).toBeVisible()

  // an address, with and without link text
  await page.getByText('see the manual').click()
  await page.keyboard.press('End')
  await page.keyboard.type(' /file')
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveText(/Link a file/)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await picker.getByTestId('file-address').click()
  await picker.getByLabel('Address').fill('javascript:alert(1)')
  await picker.getByRole('button', { name: 'Insert' }).click()
  await expect(picker.getByRole('alert')).toContainText('must start with')
  await picker.getByLabel('Address').fill('https://example.org/files/report%20final.pdf')
  await picker.getByRole('button', { name: 'Insert' }).click()
  await expect(picker).toBeHidden()
  await expect(page.getByTestId('block-editor')).toContainText(
    '[report final.pdf](https://example.org/files/report%20final.pdf)',
  )
})

test('servers are listed in Settings → Linked files and can be removed', async ({ backend, page, dav }) => {
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Linked files' }).click()
  const list = page.getByTestId('file-sources')
  await expect(list).toContainText('No servers yet.')
  await page.getByTestId('file-source-add').click()
  const form = page.getByTestId('file-source-form')
  await form.getByLabel('Name', { exact: true }).fill('Cloud')
  await form.getByLabel('Server', { exact: true }).fill(dav.server)
  await form.getByLabel('User name').fill(dav.user)
  await form.getByLabel(/^Password/).fill(dav.password)
  await form.getByRole('button', { name: 'Save' }).click()
  await expect(list).toContainText('Cloud')
  await expect(list).toContainText(dav.user)
  // the password is in the keyring (memory here), not in the settings file
  expect(JSON.stringify(await backend.call('file_sources', {}))).not.toContain(dav.password)
  await list.getByRole('button', { name: 'Edit' }).click()
  await form.getByLabel('Name', { exact: true }).fill('Work cloud')
  await form.getByRole('button', { name: 'Save' }).click() // the stored password is kept
  await expect(list).toContainText('Work cloud')
  await list.getByRole('button', { name: 'Remove…' }).click()
  await list.getByRole('button', { name: 'Remove', exact: true }).click()
  await expect(list).toContainText('No servers yet.')
})
