import { spawn, type ChildProcess } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test as base } from './fixtures'

// The in-memory WebDAV server of the sync tests as a fixture, and the helpers the Nextcloud specs share (NA-1..NA-8).
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')

export interface Dav {
  server: string
  user: string
  password: string
  /** Stops the server (the next request finds nobody). */
  stop(): void
}

export const test = base.extend<{ dav: Dav }>({
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
    await use({
      server: url.replace(/\/dav$/, ''),
      user,
      password: pw.join(' '),
      stop: () => proc.kill(),
    })
    proc.kill()
  },
})

export const auth = (dav: Dav) => 'Basic ' + Buffer.from(`${dav.user}:${dav.password}`).toString('base64')
export const account = (dav: Dav) => `${dav.server}/remote.php/dav/files/${dav.user}`

/** Puts a file on the server (the folders above it first); a second call with the same path changes the file. */
export async function put(dav: Dav, file: string, body: Buffer | string) {
  const parts = file.split('/')
  for (let i = 1; i < parts.length; i++) {
    await fetch(`${account(dav)}/${parts.slice(0, i).map(encodeURIComponent).join('/')}`, {
      method: 'MKCOL',
      headers: { Authorization: auth(dav) },
    })
  }
  await fetch(`${account(dav)}/${parts.map(encodeURIComponent).join('/')}`, {
    method: 'PUT',
    headers: { Authorization: auth(dav) },
    body: new Uint8Array(typeof body === 'string' ? Buffer.from(body) : body),
  })
}

/** Nextcloud's number for a file: what a `/f/<number>` link carries. */
export async function fileId(dav: Dav, file: string): Promise<string> {
  const r = await fetch(`${account(dav)}/${file.split('/').map(encodeURIComponent).join('/')}`, {
    method: 'PROPFIND',
    headers: { Authorization: auth(dav), Depth: '0' },
  })
  const m = /<oc:fileid>(\d+)</.exec(await r.text())
  if (!m) throw new Error(`no number for ${file}`)
  return m[1]
}

export type Backend = Parameters<Parameters<typeof base>[2]>[0]['backend']

/** Connects the graph of the test to the server: from now on the app works on the replica folder. */
export async function synced(backend: Backend, dav: Dav) {
  const info = await backend.call<{ root: string }>('create_remote_graph', {
    server: dav.server,
    user: dav.user,
    password: dav.password,
    folder: 'Notes/graph',
    attach: false,
  })
  const dir = info.root
  return {
    dir,
    write(rel: string, text: string) {
      fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true })
      fs.writeFileSync(path.join(dir, rel), text)
    },
    read: (rel: string) => fs.readFileSync(path.join(dir, rel), 'utf8'),
    exists: (rel: string) => fs.existsSync(path.join(dir, rel)),
  }
}
