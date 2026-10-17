// The sync of the open graph with its server folder (SY-1..SY-8): the status the backend reports, kept fresh by
// asking every few seconds while the graph has a server. The backend syncs on its own (a few seconds after a
// change, every minute, when the app closes); this only shows it and lets the person ask for a sync now.
import { backend, type SyncStatus } from '../api'
import { app, errorMessage, toast } from './app.svelte'

export const sync = $state({
  status: null as SyncStatus | null,
  /** A sync asked for from the UI is running. */
  busy: false,
})

let timer: ReturnType<typeof setInterval> | undefined

/** Reads the status now. */
export async function refreshSync() {
  if (!app.graph?.remote) {
    sync.status = null
    return
  }
  try {
    sync.status = await backend.call('sync_status', {})
  } catch {
    /* the graph was closed meanwhile */
  }
}

/** Starts asking for the status (no-op without a server folder); returns the function that stops it. */
export function watchSync(): () => void {
  void refreshSync()
  clearInterval(timer)
  timer = setInterval(() => void refreshSync(), 4000)
  return () => clearInterval(timer)
}

/** Syncs now and shows the result. `allowMassDelete` goes ahead with a sync that was stopped for deleting most
 * of the graph. */
export async function syncNow(allowMassDelete = false) {
  sync.busy = true
  try {
    sync.status = await backend.call('sync_now', { allow_mass_delete: allowMassDelete })
  } catch (e) {
    toast(errorMessage(e), 'error')
  } finally {
    sync.busy = false
  }
}

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
