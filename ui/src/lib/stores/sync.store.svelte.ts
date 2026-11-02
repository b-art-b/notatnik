// The sync of one graph with its server folder (SY-1..SY-8): the status the backend reports, kept fresh by asking every
// few seconds while the graph has a server. The backend syncs on its own (a few seconds after a change, every minute,
// when the app closes); this only shows it, lets the person ask for a sync now, and asks for one when the app comes back
// to the foreground (SY-10).
import type { Backend, GraphInfo, SyncStatus } from '../api'
import { errorMessage, toast } from './app.svelte'

/** A sync that ended less than this long ago counts as "just now": coming back to the app then does not sync again. */
const JUST_SYNCED_MS = 5000

export class SyncStore {
  status = $state<SyncStatus | null>(null)
  /** A sync asked for from the UI is running. */
  busy = $state(false)
  private timer: ReturnType<typeof setInterval> | undefined

  constructor(
    private readonly backend: Backend,
    private readonly info: () => GraphInfo | null,
  ) {}

  /** Reads the status now. */
  async refresh() {
    if (!this.info()?.remote) {
      this.status = null
      return
    }
    try {
      this.status = await this.backend.call('sync_status', {})
    } catch {
      /* the graph was closed meanwhile */
    }
  }

  /** Starts asking for the status (no-op without a server folder) and for a sync whenever the page becomes visible
   * again; returns the function that stops both. */
  watch(): () => void {
    void this.refresh()
    clearInterval(this.timer)
    this.timer = setInterval(() => void this.refresh(), 4000)
    // the same function object every time, so adding it twice is one listener and removing it removes that one
    document.addEventListener('visibilitychange', this.onVisibility)
    return () => {
      clearInterval(this.timer)
      document.removeEventListener('visibilitychange', this.onVisibility)
    }
  }

  // an arrow function keeps `this` (a method passed to addEventListener would lose it)
  private readonly onVisibility = () => {
    if (document.visibilityState === 'visible') void this.resume()
  }

  /** The app came back to the foreground (a phone unlocked or switched back to, the window shown again): sync now, so
   * what another device changed meanwhile is here within seconds and not at the next minute. Quiet: an offline phone
   * is not greeted with an error, the status line says so. Not while a sync runs or just ended, and not while a
   * login is refused (asking again would only count as one more failed attempt on the server). */
  async resume() {
    if (!this.info()?.remote || this.busy) return
    const s = this.status
    if (s?.state === 'syncing' || s?.error_code === 'auth') return
    if (s?.last_sync && Date.now() - Date.parse(s.last_sync) < JUST_SYNCED_MS) return
    await this.now(false, true)
  }

  /** Syncs now and shows the result. `allowMassDelete` goes ahead with a sync that was stopped for deleting most
   * of the graph. `quiet` shows no error message (the status line still says what happened). */
  async now(allowMassDelete = false, quiet = false) {
    this.busy = true
    try {
      this.status = await this.backend.call('sync_now', { allow_mass_delete: allowMassDelete })
    } catch (e) {
      if (quiet) await this.refresh()
      else toast(errorMessage(e), 'error')
    } finally {
      this.busy = false
    }
  }
}
