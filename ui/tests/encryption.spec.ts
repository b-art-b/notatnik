import { spawn, type ChildProcess } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test as base } from './fixtures'

// Encrypting chosen pages on the server (EN-11 to EN-18), against the in-memory WebDAV server of the sync tests.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const SECRET = 'SECRETMARKER-4711'

interface Dav {
  server: string
  user: string
  password: string
  /** A file on the server as its bytes, or null. */
  file(rel: string): Promise<string | null>
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
    const server = url.replace(/\/dav$/, '')
    const password = pw.join(' ')
    const auth = 'Basic ' + Buffer.from(`${user}:${password}`).toString('base64')
    await use({
      server,
      user,
      password,
      file: async (rel) => {
        const r = await fetch(`${server}/remote.php/dav/files/${user}/${rel}`, {
          headers: { Authorization: auth },
        })
        return r.ok ? await r.text() : null
      },
    })
    proc.kill()
  },
})

async function graphOnServer(
  backend: { call: <T>(n: string, a?: unknown) => Promise<T> },
  dav: Dav,
  folder: string,
) {
  const info = await backend.call<{ root: string }>('create_remote_graph', {
    server: dav.server,
    user: dav.user,
    password: dav.password,
    folder,
    attach: false,
  })
  fs.mkdirSync(path.join(info.root, 'pages'), { recursive: true })
  return info.root
}

test('a page is encrypted from its menu: the lock shows and the server holds no text of it', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/')
  const dir = await graphOnServer(backend, dav, 'Notes/enc')
  fs.writeFileSync(path.join(dir, 'pages/Secret.md'), `- ${SECRET} one\n`)
  fs.writeFileSync(path.join(dir, 'pages/Open.md'), '- open text\n')
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/enc/pages/Secret.md')).toContain(SECRET) // plain so far
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.reload()

  // set up: password twice, the recovery key is shown once and has to be confirmed
  await page.getByRole('tab', { name: /Sync/ }).click()
  await expect(page.getByTestId('enc-state')).toHaveText('Not set up')
  await page.getByTestId('graph-enc-setup').click()
  await page.getByLabel('Password (at least 8 characters)').fill('correct horse')
  await page.getByLabel('Password again').fill('correct horse')
  await page.getByRole('button', { name: 'Create' }).click()
  await expect(page.getByTestId('graph-recovery-key')).toContainText('-')
  await expect(page.getByTestId('graph-enc-confirm')).toBeDisabled()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')

  // the page: the dialog warns that the server has the text from earlier syncs
  await page.goto(page.url().replace(/#.*/, '#/page/Secret'))
  await expect(page.getByTestId('title-lock')).toHaveCount(0)
  await page.getByTestId('encrypt-toggle').click()
  await expect(page.getByTestId('encrypt-warn-synced')).toBeVisible()
  await page.getByTestId('encrypt-start').click()
  await expect(page.getByTestId('title-lock')).toBeVisible()
  await expect(page.getByTestId('encrypt-toggle')).toHaveText('Encrypted')

  // the next sync stores it encrypted; the plain page is untouched
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/enc/pages/Secret.md')).toBeNull()
  const sealed = await dav.file('Notes/enc/pages/Secret.md.enc')
  expect(sealed).not.toBeNull()
  expect(sealed).not.toContain(SECRET)
  expect(await dav.file('Notes/enc/pages/Open.md')).toContain('open text')
  expect(fs.readFileSync(path.join(dir, 'pages/Secret.md'), 'utf8')).toContain(SECRET)
  await page.goto(page.url().replace(/#.*/, '#/page/Open'))
  await expect(page.getByTestId('title-lock')).toHaveCount(0)

  // Settings lists the mark; locking stops the file, and the sidebar says so
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.getByRole('tab', { name: /Sync/ }).click()
  await expect(page.getByTestId('enc-paths')).toContainText('pages/Secret.md')
  await page.getByTestId('graph-enc-lock').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Locked')
  fs.writeFileSync(path.join(dir, 'pages/Secret.md'), `- ${SECRET} two\n`)
  await backend.call('sync_now', { allow_mass_delete: false })
  await expect(page.getByTestId('sync-locked')).toContainText('1 encrypted file')
  expect(await dav.file('Notes/enc/pages/Secret.md')).toBeNull()
  // unlocking sends the edit
  await page.getByTestId('graph-enc-unlock').click()
  await page.getByLabel('Password or recovery key').fill('correct horse')
  await page.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  await backend.call('sync_now', { allow_mass_delete: false })
  await expect(page.getByTestId('sync-locked')).toHaveCount(0)
  expect(await dav.file('Notes/enc/pages/Secret.md.enc')).not.toContain(SECRET)

  // stopping stores it as it was
  await page.goto(page.url().replace(/#.*/, '#/page/Secret'))
  await page.getByTestId('encrypt-toggle').click()
  await page.getByTestId('encrypt-stop').click()
  await expect(page.getByTestId('title-lock')).toHaveCount(0)
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/enc/pages/Secret.md.enc')).toBeNull()
  expect(await dav.file('Notes/enc/pages/Secret.md')).toContain(`${SECRET} two`)
})

test('without a vault or with a locked key the dialog sends you to Settings', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/')
  const dir = await graphOnServer(backend, dav, 'Notes/enc2')
  fs.writeFileSync(path.join(dir, 'pages/Secret.md'), '- one\n')
  await backend.call('sync_now', { allow_mass_delete: false })
  await page.goto(page.url().replace(/#.*/, '#/page/Secret'))
  await page.reload()
  await page.getByTestId('encrypt-toggle').click()
  await expect(page.getByTestId('encrypt-dialog')).toContainText('not set up')
  await expect(page.getByTestId('encrypt-goto-settings')).toBeVisible()
  await expect(page.getByTestId('encrypt-start')).toHaveCount(0)
})

test('encryption is set up and a page marked on a local graph; connecting uploads it encrypted', async ({
  backend,
  page,
  dav,
}) => {
  backend.write('pages/Secret.md', `- ${SECRET} one\n`)
  backend.write('pages/Open.md', '- open text\n')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Sync/ }).click()
  await page.getByTestId('graph-enc-setup').click()
  await page.getByLabel('Password (at least 8 characters)').fill('correct horse')
  await page.getByLabel('Password again').fill('correct horse')
  await page.getByRole('button', { name: 'Create' }).click()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')

  await page.goto(page.url().replace(/#.*/, '#/page/Secret'))
  await page.getByTestId('encrypt-toggle').click()
  await expect(page.getByTestId('encrypt-no-server')).toBeVisible()
  await expect(page.getByTestId('encrypt-warn-synced')).toHaveCount(0)
  await page.getByTestId('encrypt-start').click()
  await expect(page.getByTestId('title-lock')).toBeVisible()
  expect(await dav.file('Notes/early/pages/Secret.md.enc')).toBeNull() // nothing is uploaded yet

  await backend.call('connect_remote', {
    server: dav.server,
    user: dav.user,
    password: dav.password,
    folder: 'Notes/early',
    attach: false,
  })
  expect(await dav.file('Notes/early/pages/Open.md')).toContain('open text')
  expect(await dav.file('Notes/early/pages/Secret.md')).toBeNull()
  const sealed = await dav.file('Notes/early/pages/Secret.md.enc')
  expect(sealed).not.toBeNull()
  expect(sealed).not.toContain(SECRET)
})
