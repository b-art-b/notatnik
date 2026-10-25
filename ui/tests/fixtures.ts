import { fileURLToPath } from 'node:url'
import { test as base, expect, type Page } from '@playwright/test'
import { spawn, type ChildProcess } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const bin = path.join(root, 'target/debug/notatnik-dev-server')

function copyDir(from: string, to: string) {
  fs.mkdirSync(to, { recursive: true })
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    const s = path.join(from, e.name)
    if (e.isDirectory()) copyDir(s, path.join(to, e.name))
    else fs.copyFileSync(s, path.join(to, e.name))
  }
}

export interface Backend {
  /** Graph folder on disk (a temp copy). */
  dir: string
  api: string
  read(rel: string): string
  write(rel: string, text: string): void
  exists(rel: string): boolean
  call<T = unknown>(name: string, args?: unknown): Promise<T>
  open(route?: string): Promise<void>
  /** Opens a second graph (a copy of `fixture`, or an empty graph) in slot 1 of the same app (MG-1). */
  beside(name?: string, fixture?: string): Promise<Beside>
}

/** The graph in slot 1: what the tests need to look at it. */
export interface Beside {
  dir: string
  slot: string
  read(rel: string): string
  write(rel: string, text: string): void
  exists(rel: string): boolean
  call<T = unknown>(name: string, args?: unknown): Promise<T>
}

async function startBackend(
  fixture: string,
  today: string,
  page: Page,
  extra: string[] = [],
): Promise<{ b: Backend; stop: () => void }> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'notatnik-e2e-'))
  const dir = path.join(tmp, 'graph')
  copyDir(path.join(root, 'fixtures/graphs', fixture), dir)
  const proc: ChildProcess = spawn(
    bin,
    ['--graph', dir, '--today', today, '--device-dir', path.join(tmp, 'device'), ...extra],
    {
      stdio: ['ignore', 'pipe', 'inherit'],
      env: { ...process.env, RUST_LOG: process.env.RUST_LOG ?? 'warn' },
    },
  )
  const api = await new Promise<string>((resolve, reject) => {
    let buf = ''
    proc.stdout!.on('data', (d) => {
      buf += d
      const m = /listening on (http:\/\/\S+)/.exec(buf)
      if (m) resolve(m[1])
    })
    proc.on('exit', (c) => reject(new Error(`dev server exited early (${c})`)))
  })
  const call = async <T>(name: string, args: unknown = {}) => {
    const r = await fetch(`${api}/api/${name}`, { method: 'POST', body: JSON.stringify(args) })
    return (await r.json()) as T
  }
  const b: Backend = {
    dir,
    api,
    read: (rel) => fs.readFileSync(path.join(dir, rel), 'utf8'),
    write: (rel, text) => {
      fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true })
      fs.writeFileSync(path.join(dir, rel), text)
    },
    exists: (rel) => fs.existsSync(path.join(dir, rel)),
    call,
    open: async (route = '') => {
      await page.goto(`/?api=${encodeURIComponent(api)}#${route}`)
      await page.waitForSelector('main, .welcome')
    },
    beside: async (name = 'Beta', fixture) => {
      const dir2 = path.join(tmp, name)
      if (fixture) copyDir(path.join(root, 'fixtures/graphs', fixture), dir2)
      else fs.mkdirSync(dir2, { recursive: true })
      const call2 = async <T>(n: string, args: unknown = {}) => {
        const r = await fetch(`${api}/api/${n}?slot=1`, { method: 'POST', body: JSON.stringify(args) })
        return (await r.json()) as T
      }
      await call2('open_graph', { path: dir2, create: !fixture })
      return {
        dir: dir2,
        slot: '1',
        read: (rel) => fs.readFileSync(path.join(dir2, rel), 'utf8'),
        write: (rel, text) => {
          fs.mkdirSync(path.dirname(path.join(dir2, rel)), { recursive: true })
          fs.writeFileSync(path.join(dir2, rel), text)
        },
        exists: (rel) => fs.existsSync(path.join(dir2, rel)),
        call: call2,
      }
    },
  }
  return {
    b,
    stop: () => {
      proc.kill()
      fs.rmSync(tmp, { recursive: true, force: true })
    },
  }
}

type Opts = { fixture: string; today: string; fakeCalendar: boolean }
export const test = base.extend<{ backend: Backend } & Opts>({
  fixture: ['basic', { option: true }],
  today: ['2026-10-03', { option: true }],
  fakeCalendar: [false, { option: true }],
  backend: async ({ page, fixture, today, fakeCalendar }, use) => {
    const { b, stop } = await startBackend(fixture, today, page, fakeCalendar ? ['--fake-calendar'] : [])
    await use(b)
    stop()
  },
})
export { expect }
