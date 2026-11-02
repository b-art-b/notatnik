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

test('a local graph is connected to an empty server folder from Settings → Sync and uploaded in place', async ({
  backend,
  page,
  dav,
}) => {
  backend.write('pages/Mine.md', '- stays where it is\n')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Sync/ }).click()
  await page.getByTestId('connect-server').click()
  const form = page.getByTestId('webdav-form')
  await form.getByLabel('Server', { exact: true }).fill(dav.server)
  await form.getByLabel('User name').fill(dav.user)
  await form.getByLabel(/^Password/).fill(dav.password)
  // a folder with something in it is refused
  await fetch(`${dir(dav.server, dav.user, 'Taken')}/`, {
    method: 'MKCOL',
    headers: { Authorization: 'Basic ' + Buffer.from(`${dav.user}:${dav.password}`).toString('base64') },
  })
  await fetch(`${dir(dav.server, dav.user, 'Taken')}/note.txt`, {
    method: 'PUT',
    body: 'x',
    headers: { Authorization: 'Basic ' + Buffer.from(`${dav.user}:${dav.password}`).toString('base64') },
  })
  await form.getByLabel('Folder on the server').fill('Taken')
  await form.getByRole('button', { name: 'Check the folder' }).click()
  await expect(page.getByTestId('webdav-seen')).toContainText('choose an empty or a new folder')
  await expect(form.getByRole('button', { name: 'Upload this graph' })).toBeDisabled()
  // a new folder takes the graph
  await form.getByLabel('Folder on the server').fill('Notes/connected')
  await form.getByRole('button', { name: 'Check the folder' }).click()
  await expect(page.getByTestId('webdav-seen')).toContainText(
    'will be created and this graph will be uploaded',
  )
  await form.getByRole('button', { name: 'Upload this graph' }).click()
  await expect(page.getByTestId('sync-url')).toContainText(dir(dav.server, dav.user, 'Notes/connected'), {
    timeout: 15000,
  })
  await expect(page.getByTestId('connect-server')).toHaveCount(0)
  await expect
    .poll(async () => {
      const r = await fetch(`${dir(dav.server, dav.user, 'Notes/connected')}/pages/Mine.md`, {
        headers: { Authorization: 'Basic ' + Buffer.from(`${dav.user}:${dav.password}`).toString('base64') },
      })
      return r.ok ? await r.text() : r.status
    })
    .toBe('- stays where it is\n')
})

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

  // EN-20: Verify reads the server copy back; a file that only the server has (another device, not synced here
  // yet) shows up
  await page.getByRole('button', { name: 'Verify', exact: true }).click()
  const result = page.getByTestId('verify-result')
  await expect(result).toContainText('The server copy matches this device')
  const put = await fetch(`${dir(dav.server, dav.user, 'Notes/e2e')}/Elsewhere.md`, {
    method: 'PUT',
    headers: { Authorization: 'Basic ' + Buffer.from(`${dav.user}:${dav.password}`).toString('base64') },
    body: '- written on another device\n',
  })
  expect(put.ok).toBe(true)
  await page.getByRole('button', { name: 'Verify', exact: true }).click()
  await expect(result).toContainText('Only on the server (1)')
  await expect(result.getByText('Elsewhere.md')).toBeVisible()

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

// SY-10 (AS4): coming back to the app syncs at once. The engine's own timer is a minute; the test waits seconds.
test('a sync starts when the app comes back to the foreground', async ({ backend, page, dav }) => {
  await backend.open('/journals')
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('Switch graph')
  await page.keyboard.press('Enter')
  await page.getByTestId('new-server-graph').click()
  const form = page.getByTestId('webdav-form')
  await form.getByLabel('Server', { exact: true }).fill(dav.server)
  await form.getByLabel('User name').fill(dav.user)
  await form.getByLabel(/^Password/).fill(dav.password)
  await form.getByLabel('Folder on the server').fill('Notes/resume')
  await form.getByRole('button', { name: 'Check the folder' }).click()
  await form.getByRole('button', { name: 'Create the graph' }).click()
  const status = page.getByTestId('sync-status')
  await expect(status).toContainText('Synced', { timeout: 15000 })
  // settle: a sync of our own, then another device writes a file
  await status.click()
  await expect(status).toHaveAttribute('data-state', 'idle')
  const syncStatus = () => backend.call<{ last_sync: string; pulled: number }>('sync_status', {})
  // the engine syncs once more 5 s after the last change on this device (the graph was just made): let that one
  // pass, or it would fetch the file below by itself and the test could not tell
  await page.waitForTimeout(6500)
  const before = (await syncStatus()).last_sync
  const put = await fetch(`${dir(dav.server, dav.user, 'Notes/resume')}/Elsewhere.md`, {
    method: 'PUT',
    headers: { Authorization: 'Basic ' + Buffer.from(`${dav.user}:${dav.password}`).toString('base64') },
    body: '- written on another device\n',
  })
  expect(put.ok).toBe(true)
  await page.waitForTimeout(1100) // `last_sync` has a resolution of a second, and a sync that just ended is not repeated

  // the page goes to the background and comes back: the browser's page visibility events
  await page.evaluate(() => {
    const set = (v: string) => {
      Object.defineProperty(document, 'visibilityState', { value: v, configurable: true })
      document.dispatchEvent(new Event('visibilitychange'))
    }
    set('hidden')
    set('visible')
  })
  // within seconds, not at the timer of the next minute
  await expect.poll(async () => (await syncStatus()).last_sync, { timeout: 8000 }).not.toBe(before)
  expect((await syncStatus()).pulled).toBe(1)
})

// AS2 (docs/phases/android-sync.md): the same route on a phone or tablet. The app's Rust shell sets
// `window.__NOTATNIK_MOBILE__` before the page loads; the browser tests do it with an init script. No keyboard
// shortcut is used: the way to the server form is the sidebar's graph switcher.
test.describe('on a phone', () => {
  test.use({ viewport: { width: 412, height: 915 }, hasTouch: true })

  async function asPhone(page: import('@playwright/test').Page) {
    await page.addInitScript(() => {
      ;(window as unknown as { __NOTATNIK_MOBILE__: boolean }).__NOTATNIK_MOBILE__ = true
    })
    await page.reload()
    // a narrow screen starts with the sidebar closed
    const show = page.getByRole('button', { name: 'Show sidebar' })
    if (await show.isVisible()) await show.tap()
  }

  test('the server form is reached by touch, without a folder to type or choose, and a graph is downloaded', async ({
    backend,
    page,
    dav,
  }) => {
    await backend.open('/journals')
    // a graph for the phone to download: put one there through a first device
    await backend.call('create_remote_graph', {
      server: dav.server,
      user: dav.user,
      password: dav.password,
      folder: 'Notes/phone',
      attach: false,
    })
    await asPhone(page)

    await page.getByTestId('graph-switcher').tap()
    await page.getByRole('menuitem', { name: 'Add new graph…' }).tap()

    // the start screen: the graphs of this device and the server, no free folder field, no folder chooser
    await expect(page.getByRole('heading', { name: 'On a server' })).toBeVisible()
    await expect(page.getByLabel('Graph folder')).toHaveCount(0)
    await expect(page.getByRole('heading', { name: 'Another folder' })).toHaveCount(0)
    await expect(page.getByTestId('browse-folder')).toHaveCount(0)

    await page.getByTestId('new-server-graph').tap()
    const form = page.getByTestId('webdav-form')
    await form.getByLabel('Server', { exact: true }).fill(dav.server)
    await form.getByLabel('User name').fill(dav.user)
    await form.getByLabel(/^Password/).fill(dav.password)
    await form.getByLabel('Folder on the server').fill('Notes/phone')
    await form.getByRole('button', { name: 'Check the folder' }).tap()
    await expect(page.getByTestId('webdav-seen')).toContainText('already holds a Notatnik graph')
    await form.getByRole('button', { name: 'Download and open' }).tap()

    // the downloaded graph opens and is synced with the server
    await expect(page.getByTestId('journals')).toBeVisible({ timeout: 15000 })
    const show = page.getByRole('button', { name: 'Show sidebar' })
    if (await show.isVisible()) await show.tap()
    await expect(page.getByTestId('sync-status')).toContainText('Synced', { timeout: 15000 })
  })

  test('Settings does not name a path of a computer', async ({ backend, page }) => {
    await backend.open('/journals')
    await asPhone(page)
    await page.goto(page.url().replace(/#.*/, '#/settings'))
    const pane = page.getByRole('tabpanel')
    await expect(pane).toContainText("Stored in the app's own storage on this device")
    await expect(pane).not.toContainText('~/.config')
  })
})
