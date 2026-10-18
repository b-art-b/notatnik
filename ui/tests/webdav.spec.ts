import { spawn, type ChildProcess } from 'node:child_process'
import fs from 'node:fs'
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

const dir = (server: string, user: string, folder: string) =>
  `${server}/remote.php/dav/files/${user}/${folder}`

test('a new graph on a server is created, shows its sync status, and can be disconnected', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  await page.getByTestId('new-server-graph').click()
  const form = page.getByTestId('webdav-form')
  await form.getByLabel('Server', { exact: true }).fill(dav.server)
  await form.getByLabel('User name').fill(dav.user)
  await form.getByLabel(/^Password/).fill(dav.password)
  await form.getByLabel('Folder on the server').fill('Notes/e2e')
  await form.getByRole('button', { name: 'Check the folder' }).click()
  await expect(page.getByTestId('webdav-seen')).toContainText('does not exist yet')
  await form.getByRole('button', { name: 'Create the graph' }).click()

  // the new graph opens (the page reloads for another graph) and says it lives on a server
  const status = page.getByTestId('sync-status')
  await expect(status).toBeVisible({ timeout: 15000 })
  await expect(status).toContainText('Synced')
  await status.click()
  await expect(status).toHaveAttribute('data-state', 'idle')
  // the tooltip says it is a button, and the same action is in the palette
  await expect(status).toHaveAttribute('title', /Click to sync now/)
  const before = (await backend.call<{ last_sync: string }>('sync_status', {})).last_sync
  await page.waitForTimeout(1100) // `last_sync` has a resolution of a second
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Sync now')
  await page.keyboard.press('Enter')
  await expect
    .poll(async () => (await backend.call<{ last_sync: string }>('sync_status', {})).last_sync)
    .not.toBe(before)

  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.getByRole('tab', { name: /Sync/ }).click()
  await expect(page.getByTestId('sync-url')).toContainText(dir(dav.server, dav.user, 'Notes/e2e'))
  await expect(page.getByTestId('sync-user')).toHaveText(dav.user)
  await expect(page.getByTestId('sync-line')).toContainText('Synced')

  // disconnect: the graph stays, the indicator and the pane go back to "local"
  await page.getByRole('button', { name: 'Disconnect…' }).click()
  await page.getByRole('button', { name: 'Disconnect', exact: true }).click()
  await expect(page.getByTestId('journals')).toBeVisible({ timeout: 10000 })
  await expect(page.getByTestId('sync-status')).toHaveCount(0)
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.getByRole('tab', { name: /Sync/ }).click()
  await expect(page.getByText('This graph is a folder on this device only.')).toBeVisible()
})

test('a folder that holds a graph is downloaded; other folders are refused', async ({
  backend,
  page,
  dav,
}) => {
  // the folder holds a graph already: put one there through a first device (the API of the same server)
  await backend.open('/journals')
  const first = await backend.call<{ id: string; root: string }>('create_remote_graph', {
    server: dav.server,
    user: dav.user,
    password: dav.password,
    folder: 'Notes/shared',
    attach: false,
  })
  expect(first.id).toBeTruthy()

  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  await page.getByTestId('new-server-graph').click()
  const form = page.getByTestId('webdav-form')
  await form.getByLabel('Server', { exact: true }).fill(dav.server)
  await form.getByLabel('User name').fill(dav.user)
  await form.getByLabel(/^Password/).fill(dav.password)
  await form.getByLabel('Folder on the server').fill('Notes/shared')
  await form.getByRole('button', { name: 'Check the folder' }).click()
  await expect(page.getByTestId('webdav-seen')).toContainText('already holds a Notatnik graph')
  await expect(form.getByRole('button', { name: 'Download and open' })).toBeVisible()

  // a wrong password is said plainly
  await form.getByLabel(/^Password/).fill('nope')
  await form.getByLabel('Folder on the server').fill('Notes/other')
  await form.getByRole('button', { name: 'Check the folder' }).click()
  await expect(form.getByRole('alert')).toContainText('refused the login')
  expect(fs.existsSync(first.root)).toBe(true)
})

test('the folder browser clicks through the server and fills in the folder', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/journals')
  // something to browse: a graph in Notes/Work/graph1
  await backend.call('create_remote_graph', {
    server: dav.server,
    user: dav.user,
    password: dav.password,
    folder: 'Notes/Work/graph1',
    attach: false,
  })
  const login = { server: dav.server, user: dav.user, password: dav.password, attach: false }
  await expect
    .poll(
      async () =>
        (await backend.call<{ folders: string[] }>('webdav_browse', { ...login, folder: '' })).folders,
    )
    .toContain('Notes')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  await page.getByTestId('new-server-graph').click()
  const form = page.getByTestId('webdav-form')
  // not before the login is filled in
  await expect(form.getByTestId('webdav-browse')).toBeDisabled()
  await form.getByLabel('Server', { exact: true }).fill(dav.server)
  await form.getByLabel('User name').fill(dav.user)
  await form.getByLabel(/^Password/).fill(dav.password)
  await form.getByLabel('Folder on the server').fill('')
  await form.getByTestId('webdav-browse').click()
  const browser = page.getByTestId('webdav-browser')
  await expect(browser.getByRole('button', { name: /Notes/ })).toBeVisible()
  await browser.getByRole('button', { name: /Notes/ }).click()
  await browser.getByRole('button', { name: /Work/ }).click()
  await expect(browser.getByRole('button', { name: /graph1/ })).toBeVisible()
  // the crumbs go back up
  await browser
    .getByRole('navigation', { name: 'Where you are' })
    .getByRole('button', { name: 'Notes' })
    .click()
  await expect(browser.getByRole('button', { name: /Work/ })).toBeVisible()
  await browser.getByRole('button', { name: /Work/ }).click()
  // a folder that is not there yet
  await browser.getByLabel('New folder here').fill('mine')
  await browser.getByRole('button', { name: 'Add' }).click()
  await expect(browser).toContainText('does not exist yet')
  await browser.getByTestId('browser-pick').click()
  await expect(form.getByLabel('Folder on the server')).toHaveValue('Notes/Work/mine')
  await expect(page.getByTestId('webdav-seen')).toContainText('does not exist yet')
  await expect(browser).toHaveCount(0)
})

test('a page that another editor rewrote with its lines joined is reported in the sidebar and in Settings', async ({
  backend,
  page,
  dav,
}) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  await page.getByTestId('new-server-graph').click()
  const form = page.getByTestId('webdav-form')
  await form.getByLabel('Server', { exact: true }).fill(dav.server)
  await form.getByLabel('User name').fill(dav.user)
  await form.getByLabel(/^Password/).fill(dav.password)
  await form.getByLabel('Folder on the server').fill('Notes/joined')
  await form.getByRole('button', { name: 'Check the folder' }).click()
  await form.getByRole('button', { name: 'Create the graph' }).click()
  const status = page.getByTestId('sync-status')
  await expect(status).toContainText('Synced', { timeout: 15000 })
  await expect(page.getByTestId('sync-warning')).toHaveCount(0)

  // something else (a web editor) saves a page on the server with the lines of a block joined
  const auth = `Basic ${Buffer.from(`${dav.user}:${dav.password}`).toString('base64')}`
  const folder = `${dir(dav.server, dav.user, 'Notes/joined')}/pages`
  await fetch(folder, { method: 'MKCOL', headers: { Authorization: auth } }) // may exist already
  const url = `${folder}/Joined.md`
  const put = await fetch(url, {
    method: 'PUT',
    headers: { Authorization: auth },
    body: '- TODO call Anna ^abc123 deadline:: 2026-10-21',
  })
  expect(put.ok).toBe(true)
  await status.click()
  await expect(page.getByTestId('sync-warning')).toContainText('1 page(s) look reformatted')
  await page.goto(page.url().replace(/#.*/, '#/settings'))
  await page.getByRole('tab', { name: /Sync/ }).click()
  await expect(page.getByTestId('sync-warnings')).toContainText('pages/Joined.md')
  await expect(page.getByTestId('sync-warnings')).toContainText('History')
})
