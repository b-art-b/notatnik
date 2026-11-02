import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Backend, GraphInfo, SyncStatus } from '../api'
import { syncLine } from './sync.svelte'
import { SyncStore } from './sync.store.svelte'

const base: SyncStatus = {
  enabled: true,
  state: 'idle',
  error_code: null,
  error: null,
  last_sync: null,
  pushed: 0,
  pulled: 0,
  deleted: 0,
  conflicts: 0,
  merged: 0,
  merge: true,
  pending_deletes: 0,
  file_errors: [],
  warnings: [],
  left_alone: 0,
  reserved: [],
  url: '',
  user: '',
}
const t0 = Date.parse('2026-10-17T12:00:00Z')

describe('syncLine', () => {
  it('says how long ago the last sync was', () => {
    const at = (ago: number) => ({ ...base, last_sync: new Date(t0 - ago * 60000).toISOString() })
    expect(syncLine(at(0), t0)).toBe('Synced just now')
    expect(syncLine(at(5), t0)).toBe('Synced 5 min ago')
    expect(syncLine(at(180), t0)).toBe('Synced 3 h ago')
    expect(syncLine(at(60 * 50), t0)).toBe('Synced 2 d ago')
    expect(syncLine(base, t0)).toBe('Not synced yet')
  })
  it('puts a problem before the time', () => {
    const when = new Date(t0).toISOString()
    expect(syncLine({ ...base, last_sync: when, state: 'syncing' }, t0)).toBe('Syncing…')
    expect(syncLine({ ...base, last_sync: when, state: 'error', error_code: 'auth' }, t0)).toBe(
      'Sign in needed',
    )
    expect(syncLine({ ...base, last_sync: when, state: 'offline', error_code: 'offline' }, t0)).toBe(
      'Offline',
    )
    expect(syncLine({ ...base, state: 'error', error_code: 'mass_delete' }, t0)).toBe(
      'Stopped: many deletions',
    )
    expect(syncLine({ ...base, state: 'error', error_code: 'files' }, t0)).toBe('Sync problem')
  })
})

// SY-10: the app asks for a sync when it comes back to the foreground
describe('SyncStore.resume', () => {
  const ago = (ms: number): SyncStatus => ({ ...base, last_sync: new Date(Date.now() - ms).toISOString() })

  /** A store over a fake backend that records the commands it is asked for. */
  function store(status: SyncStatus | null, opts: { remote?: boolean; failSync?: boolean } = {}) {
    const calls: string[] = []
    const backend = {
      call: vi.fn(async (name: string) => {
        calls.push(name)
        if (name === 'sync_now' && opts.failSync) throw new Error('offline')
        return name === 'sync_now' ? ago(0) : base
      }),
    } as unknown as Backend
    const info = { remote: opts.remote ?? true } as unknown as GraphInfo
    const s = new SyncStore(backend, () => info)
    s.status = status
    return { s, calls }
  }
  const setVisibility = (v: 'visible' | 'hidden') => {
    Object.defineProperty(document, 'visibilityState', { value: v, configurable: true })
    document.dispatchEvent(new Event('visibilitychange'))
  }
  afterEach(() => setVisibility('visible'))

  it('syncs when the last sync is old, or there was none', async () => {
    const old = store(ago(60_000))
    await old.s.resume()
    expect(old.calls).toEqual(['sync_now'])
    const none = store(base)
    await none.s.resume()
    expect(none.calls).toEqual(['sync_now'])
  })

  it('does not sync again right after a sync, while one runs, while a login is refused, or without a server', async () => {
    const cases = [
      store(ago(1000)),
      store({ ...ago(60_000), state: 'syncing' }),
      store({ ...ago(60_000), state: 'error', error_code: 'auth' }),
      store(ago(60_000), { remote: false }),
    ]
    for (const c of cases) {
      await c.s.resume()
      expect(c.calls).toEqual([])
    }
  })

  it('is quiet when the sync fails: the status is read instead of an error shown', async () => {
    const c = store(ago(60_000), { failSync: true })
    await expect(c.s.resume()).resolves.toBeUndefined()
    expect(c.calls).toEqual(['sync_now', 'sync_status'])
    expect(c.s.busy).toBe(false)
  })

  it('is started by the page becoming visible, not hidden, and stops with the watch', async () => {
    const c = store(ago(60_000))
    const stop = c.s.watch()
    await vi.waitFor(() => expect(c.calls).toContain('sync_status')) // the first status read of the watch
    c.calls.length = 0
    setVisibility('hidden')
    expect(c.calls).toEqual([])
    setVisibility('visible')
    await vi.waitFor(() => expect(c.calls).toContain('sync_now'))
    stop()
    c.calls.length = 0
    c.s.status = ago(60_000)
    setVisibility('visible')
    await new Promise((r) => setTimeout(r, 20))
    expect(c.calls).toEqual([])
  })
})
