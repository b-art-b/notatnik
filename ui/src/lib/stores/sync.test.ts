import { describe, expect, it } from 'vitest'
import type { SyncStatus } from '../api'
import { syncLine } from './sync.svelte'

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
