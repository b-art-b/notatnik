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
  await page.getByLabel('Password (8 to 200 characters)').fill('correct horse')
  await page.getByLabel('Password again').fill('correct horse')
  await page.getByTestId('enc-scope-marked').check() // these tests mark paths themselves
  await page.getByRole('button', { name: 'Create' }).click()
  await expect(page.getByTestId('graph-recovery-key')).toContainText('-')
  await expect(page.getByTestId('graph-enc-confirm')).toBeDisabled()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  // these tests look at the visible `.enc` names; the hidden ones have their own test below
  await backend.call('encryption_names', { hidden: false })

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
  await page.getByLabel('Password (8 to 200 characters)').fill('correct horse')
  await page.getByLabel('Password again').fill('correct horse')
  await page.getByTestId('enc-scope-marked').check() // these tests mark paths themselves
  await page.getByRole('button', { name: 'Create' }).click()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  // these tests look at the visible `.enc` names; the hidden ones have their own test below
  await backend.call('encryption_names', { hidden: false })

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

test('All pages shows a lock on the pages that are stored encrypted', async ({ backend, page }) => {
  backend.write('pages/Secret.md', '- hidden\n')
  backend.write('pages/vault/Deep.md', '- deep\n')
  backend.write('pages/Open.md', '- open\n')
  await backend.open('/')
  await backend.call('encryption_setup', { password: 'correct horse' })
  await backend.call('encryption_confirm', {})
  await backend.call('encryption_mark', { paths: ['pages/Secret.md', 'pages/vault/'], encrypted: true })
  await page.evaluate(() => (location.hash = '#/pages'))
  const row = (name: string) =>
    page.getByTestId('pages-table').locator('tr', { has: page.locator(`a[data-page="${name}"]`) })
  await expect(row('Secret').getByTestId('row-lock')).toBeVisible()
  await expect(row('vault/Deep').getByTestId('row-lock')).toBeVisible()
  await expect(row('Open')).toBeVisible()
  await expect(row('Open').getByTestId('row-lock')).toHaveCount(0)

  // the filter keeps only the encrypted ones
  await page.getByTestId('only-encrypted').check()
  await expect(row('Open')).toHaveCount(0)
  await expect(row('Secret')).toBeVisible()
  await expect(row('vault/Deep')).toBeVisible()
  await expect(page.getByTestId('pages-count')).toHaveText('2')

  // the Namespaces view shows the lock too
  await page.getByTestId('only-encrypted').uncheck()
  await page.getByRole('button', { name: 'Namespaces' }).click()
  const tree = page.getByTestId('pages-tree')
  const line = (name: string) => tree.locator('.line', { has: page.locator(`a[data-ns="${name}"]`) })
  await expect(line('Secret').getByTestId('row-lock')).toBeVisible()
  await expect(line('Open').getByTestId('row-lock')).toHaveCount(0)
  await line('vault').getByRole('button', { name: 'Expand' }).click()
  await expect(line('vault/Deep').getByTestId('row-lock')).toBeVisible()

  // "Only encrypted" in the tree: unencrypted pages go, the folder above an encrypted page stays and is open
  await page.getByTestId('only-encrypted').check()
  await expect(line('Open')).toHaveCount(0)
  await expect(line('Secret')).toBeVisible()
  await expect(line('vault/Deep')).toBeVisible()

  // the sidebar's page tree has the lock as well
  const side = page.getByRole('complementary', { name: 'Sidebar' })
  const sideLine = (name: string) => side.locator('.line', { has: page.locator(`a[data-ns="${name}"]`) })
  await expect(sideLine('Secret').getByTestId('row-lock')).toBeVisible()
  await expect(sideLine('Open')).toBeVisible()
  await expect(sideLine('Open').getByTestId('row-lock')).toHaveCount(0)

  // Recent: open a page of each kind
  await page.evaluate(() => (location.hash = '#/page/Secret'))
  await page.evaluate(() => (location.hash = '#/page/Open'))
  const recent = page.getByTestId('recent-pages')
  const recentItem = (name: string) => recent.locator('li', { has: page.locator(`a[data-recent="${name}"]`) })
  await expect(recentItem('Secret').getByTestId('recent-lock')).toBeVisible()
  await expect(recentItem('Open')).toBeVisible()
  await expect(recentItem('Open').getByTestId('recent-lock')).toHaveCount(0)
})

test('Encrypt everything switches the scope in one step; the settings stay readable', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/')
  const dir = await graphOnServer(backend, dav, 'Notes/all')
  fs.mkdirSync(path.join(dir, 'journals'), { recursive: true })
  fs.writeFileSync(path.join(dir, 'pages/A.md'), `- ${SECRET} a\n`)
  fs.writeFileSync(path.join(dir, 'pages/B.md'), `- ${SECRET} b\n`)
  fs.writeFileSync(path.join(dir, 'journals/2026_10_01.md'), `- ${SECRET} day\n`)
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/all/pages/A.md')).toContain(SECRET) // plain so far
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.reload()
  await page.getByRole('tab', { name: /Sync/ }).click()

  // not before the key exists and is unlocked
  await expect(page.getByTestId('enc-all')).toHaveCount(0)
  await page.getByTestId('graph-enc-setup').click()
  await page.getByLabel('Password (8 to 200 characters)').fill('correct horse')
  await page.getByLabel('Password again').fill('correct horse')
  await page.getByTestId('enc-scope-marked').check() // these tests mark paths themselves
  await page.getByRole('button', { name: 'Create' }).click()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  // these tests look at the visible `.enc` names; the hidden ones have their own test below
  await backend.call('encryption_names', { hidden: false })

  // the confirmation says how much, and warns that the server has the text from before
  await page.getByTestId('enc-all').click()
  await expect(page.getByTestId('enc-all-form')).toContainText('2 pages') // counted from the index: the journal was written after the graph opened
  await expect(page.getByTestId('enc-all-warn-synced')).toBeVisible()
  await page.getByTestId('enc-all-start').click()
  await expect(page.getByTestId('enc-all-done')).toBeVisible()
  await expect(page.getByTestId('enc-paths')).toContainText('Plain on the server')
  await expect(page.getByTestId('enc-paths')).toContainText('Nothing: all files are encrypted')

  // the next sync stores every page encrypted, the settings stay plain
  await backend.call('sync_now', { allow_mass_delete: false })
  for (const f of ['pages/A.md', 'pages/B.md', 'journals/2026_10_01.md']) {
    expect(await dav.file(`Notes/all/${f}`)).toBeNull()
    const sealed = await dav.file(`Notes/all/${f}.enc`)
    expect(sealed).not.toBeNull()
    expect(sealed).not.toContain(SECRET)
  }
  expect(await dav.file('Notes/all/.notatnik/vault.json')).not.toBeNull()
  expect(fs.readFileSync(path.join(dir, 'pages/A.md'), 'utf8')).toContain(SECRET)

  // locked: the button is there only while the key is open
  await page.getByTestId('graph-enc-lock').click()
  await expect(page.getByTestId('enc-all-done')).toBeVisible()
})

test('a forgotten graph password is reset: the new vault replaces the old, the files are stored again', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/')
  const dir = await graphOnServer(backend, dav, 'Notes/reset')
  fs.writeFileSync(path.join(dir, 'pages/Secret.md'), `- ${SECRET} one\n`)
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.reload()
  await page.getByRole('tab', { name: /Sync/ }).click()
  await page.getByTestId('graph-enc-setup').click()
  await page.getByLabel('Password (8 to 200 characters)').fill('old password')
  await page.getByLabel('Password again').fill('old password')
  await page.getByTestId('enc-scope-marked').check() // these tests mark paths themselves
  await page.getByRole('button', { name: 'Create' }).click()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  // these tests look at the visible `.enc` names; the hidden ones have their own test below
  await backend.call('encryption_names', { hidden: false })
  await backend.call('encryption_mark', { paths: ['pages/'], encrypted: true })
  await backend.call('sync_now', { allow_mass_delete: false })
  const before = await dav.file('Notes/reset/pages/Secret.md.enc')
  expect(before).not.toBeNull()
  await page.getByTestId('graph-enc-lock').click()

  // forgot: the checkbox is needed, then the new recovery key is confirmed
  await page.getByTestId('graph-enc-forgot').click()
  const form = page.getByTestId('graph-enc-reset-form')
  await form.getByLabel('New password (8 to 200 characters)').fill('new password')
  await form.getByLabel('New password again').fill('new password')
  await expect(form.getByRole('button', { name: 'Reset the password' })).toBeDisabled()
  await page.getByTestId('graph-enc-reset-understood').check()
  await form.getByRole('button', { name: 'Reset the password' }).click()
  await expect(page.getByTestId('graph-enc-confirm')).toHaveText('Replace the password')
  await expect(page.getByTestId('graph-enc-confirm')).toBeDisabled()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')

  // the file is stored again with the new key; the old password no longer unlocks
  await backend.call('sync_now', { allow_mass_delete: false })
  const after = await dav.file('Notes/reset/pages/Secret.md.enc')
  expect(after).not.toBeNull()
  expect(after).not.toBe(before)
  expect(after).not.toContain(SECRET)
  expect(fs.readFileSync(path.join(dir, 'pages/Secret.md'), 'utf8')).toContain(SECRET)
  await page.getByTestId('graph-enc-lock').click()
  await page.getByTestId('graph-enc-unlock').click()
  await page.getByLabel('Password or recovery key').fill('old password')
  await page.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: /not the password/ })).toBeVisible()
  await page.getByLabel('Password or recovery key').fill('new password')
  await page.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
})

// EN-25, EN-26: a new vault hides the names; the switch moves the files; re-encrypting replaces the key.
test('names are hidden by default, the switch moves the files, and a new key re-encrypts everything', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/')
  const dir = await graphOnServer(backend, dav, 'Notes/hid')
  fs.writeFileSync(path.join(dir, 'pages/Secret.md'), `- ${SECRET}\n`)
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.reload()
  await page.getByRole('tab', { name: /Sync/ }).click()
  await page.getByTestId('graph-enc-setup').click()
  await page.getByLabel('Password (8 to 200 characters)').fill('first password')
  await page.getByLabel('Password again').fill('first password')
  await page.getByTestId('enc-scope-marked').check() // these tests mark paths themselves
  await page.getByRole('button', { name: 'Create' }).click()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  await backend.call('encryption_mark', { paths: ['pages/'], encrypted: true })
  await backend.call('sync_now', { allow_mass_delete: false })

  // hidden by default: neither the plain name nor `<name>.enc` is on the server, and Verify reads the object
  const names = page.getByRole('switch', { name: 'Hide file and folder names' })
  await expect(names).toBeChecked()
  expect(await dav.file('Notes/hid/pages/Secret.md')).toBeNull()
  expect(await dav.file('Notes/hid/pages/Secret.md.enc')).toBeNull()
  const verify = async () => {
    await page.getByRole('button', { name: 'Verify', exact: true }).click()
    await expect(page.getByTestId('verify-result')).toContainText('The server copy matches this device')
  }
  await verify()

  // switched off: `<name>.enc`; on again: hidden
  await names.uncheck()
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/hid/pages/Secret.md.enc')).not.toBeNull()
  await names.check()
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/hid/pages/Secret.md.enc')).toBeNull()
  await verify()

  // re-encrypting needs the unlocked key; then a new password, a confirmed recovery key, and the files are stored again
  await page.getByTestId('graph-enc-rotate').click()
  const form = page.getByTestId('graph-enc-rotate-form')
  await form.getByLabel('New password (8 to 200 characters)').fill('second password')
  await form.getByLabel('New password again').fill('second password')
  await expect(form.getByRole('button', { name: 'Re-encrypt with a new key' })).toBeDisabled()
  await page.getByTestId('graph-enc-rotate-understood').check()
  await form.getByRole('button', { name: 'Re-encrypt with a new key' }).click()
  await expect(page.getByTestId('graph-enc-confirm')).toHaveText('Replace the key')
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  await backend.call('sync_now', { allow_mass_delete: false })
  await verify()
  await page.getByTestId('graph-enc-lock').click()
  await page.getByTestId('graph-enc-unlock').click()
  await page.getByLabel('Password or recovery key').fill('first password')
  await page.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: /not the password/ })).toBeVisible()
  await page.getByLabel('Password or recovery key').fill('second password')
  await page.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
})

test('everything is encrypted by default; a page can be kept plain, and the scope can go back', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/')
  const dir = await graphOnServer(backend, dav, 'Notes/scope')
  fs.mkdirSync(path.join(dir, 'pages/menu'), { recursive: true })
  fs.writeFileSync(path.join(dir, 'pages/Secret.md'), `- ${SECRET} one\n`)
  fs.writeFileSync(path.join(dir, 'pages/menu/Lunch.md'), '- soup\n')
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.reload()
  await page.getByRole('tab', { name: /Sync/ }).click()
  await page.getByTestId('graph-enc-setup').click()
  await page.getByLabel('Password (8 to 200 characters)').fill('correct horse')
  await page.getByLabel('Password again').fill('correct horse')
  // the recommended choice is already selected
  await expect(page.getByTestId('enc-scope-all')).toBeChecked()
  await page.getByRole('button', { name: 'Create' }).click()
  await page.getByTestId('graph-enc-saved').check()
  await page.getByTestId('graph-enc-confirm').click()
  await expect(page.getByTestId('enc-state')).toHaveText('Unlocked')
  await expect(page.getByTestId('enc-all-done')).toBeVisible()
  await backend.call('encryption_names', { hidden: false }) // this test looks at the `.enc` names
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/scope/pages/Secret.md.enc')).not.toBeNull()
  expect(await dav.file('Notes/scope/pages/menu/Lunch.md.enc')).not.toBeNull()
  expect(await dav.file('Notes/scope/.notatnik/vault.json')).not.toBeNull()

  // an exception from the settings: a folder
  const list = page.getByTestId('enc-paths')
  await page.getByTestId('enc-except-input').fill('pages/menu/')
  await page.getByTestId('enc-except-add').click()
  await expect(list).toContainText('pages/menu/')
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/scope/pages/menu/Lunch.md')).toContain('soup')
  expect(await dav.file('Notes/scope/pages/menu/Lunch.md.enc')).toBeNull()
  expect(await dav.file('Notes/scope/pages/Secret.md')).toBeNull()

  // and taken back
  await list.getByRole('button', { name: 'Encrypt again' }).click()
  await expect(list).toContainText('Nothing: all files are encrypted')
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/scope/pages/menu/Lunch.md')).toBeNull()
  expect(await dav.file('Notes/scope/pages/menu/Lunch.md.enc')).not.toBeNull()

  // back to only what is marked: everything is plain on the server again, the local files were never touched
  await page.getByTestId('enc-all-off').click()
  await expect(page.getByTestId('enc-all-form')).toContainText('pages will be stored as plain text')
  await page.getByTestId('enc-all-stop').click()
  await expect(page.getByTestId('enc-all')).toBeVisible()
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/scope/pages/Secret.md')).toContain(SECRET)
  expect(await dav.file('Notes/scope/pages/Secret.md.enc')).toBeNull()
  expect(fs.readFileSync(path.join(dir, 'pages/Secret.md'), 'utf8')).toContain(SECRET)
})

test('the page menu keeps one page plain while everything is encrypted', async ({ backend, page, dav }) => {
  await backend.open('/')
  const dir = await graphOnServer(backend, dav, 'Notes/keep')
  fs.writeFileSync(path.join(dir, 'pages/Secret.md'), `- ${SECRET}\n`)
  fs.writeFileSync(path.join(dir, 'pages/Shared.md'), '- for everyone\n')
  await backend.call('encryption_setup', { password: 'correct horse', scope: 'all' })
  await backend.call('encryption_confirm', {})
  await backend.call('encryption_names', { hidden: false })
  await page.goto(page.url().replace(/#.*/, '#/page/Shared'))
  await page.reload()
  await page.getByRole('button', { name: 'Encrypted', exact: true }).click()
  const dialog = page.getByTestId('encrypt-dialog')
  await expect(dialog.getByTestId('encrypt-all-note')).toBeVisible()
  await dialog.getByTestId('encrypt-keep-plain').click()
  await expect(page.getByRole('button', { name: 'Encrypt…' })).toBeVisible()
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/keep/pages/Shared.md')).toContain('for everyone')
  expect(await dav.file('Notes/keep/pages/Secret.md')).toBeNull()
  // the dialog now offers to encrypt it again
  await page.getByRole('button', { name: 'Encrypt…' }).click()
  await expect(page.getByTestId('encrypt-dialog').getByTestId('encrypt-plain')).toBeVisible()
  await page.getByTestId('encrypt-again').click()
  await backend.call('sync_now', { allow_mass_delete: false })
  expect(await dav.file('Notes/keep/pages/Shared.md')).toBeNull()
  expect(await dav.file('Notes/keep/pages/Shared.md.enc')).not.toBeNull()
})
