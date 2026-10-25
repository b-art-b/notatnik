// The sync of the active graph with its server folder (`SyncStore`; SY-1..SY-8).
import type { SyncStatus } from '../api'
import { active } from './active.svelte'

export const sync = {
  get status() {
    return active().sync.status
  },
  set status(v: SyncStatus | null) {
    active().sync.status = v
  },
  get busy() {
    return active().sync.busy
  },
}

export const refreshSync = () => active().sync.refresh()
export const watchSync = () => active().sync.watch()
export const syncNow = (allowMassDelete = false) => active().sync.now(allowMassDelete)

/** A short line for the status, e.g. "Synced 3 min ago". */
export function syncLine(s: SyncStatus, now = Date.now()): string {
  if (s.state === 'syncing') return 'Syncing…'
  if (s.error_code === 'auth') return 'Sign in needed'
  if (s.state === 'offline') return 'Offline'
  if (s.error_code === 'mass_delete') return 'Stopped: many deletions'
  if (s.state === 'error') return 'Sync problem'
  if (!s.last_sync) return 'Not synced yet'
  const mins = Math.floor((now - Date.parse(s.last_sync)) / 60000)
  if (mins < 1) return 'Synced just now'
  if (mins < 60) return `Synced ${mins} min ago`
  const hours = Math.floor(mins / 60)
  return hours < 24 ? `Synced ${hours} h ago` : `Synced ${Math.floor(hours / 24)} d ago`
}
