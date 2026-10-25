// The sync of one graph with its server folder (SY-1..SY-8): the status the backend reports, kept fresh by asking every
// few seconds while the graph has a server. The backend syncs on its own (a few seconds after a change, every minute,
// when the app closes); this only shows it and lets the person ask for a sync now.
import type { Backend, GraphInfo, SyncStatus } from '../api'
import { errorMessage, toast } from './app.svelte'

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

  /** Starts asking for the status (no-op without a server folder); returns the function that stops it. */
  watch(): () => void {
    void this.refresh()
    clearInterval(this.timer)
    this.timer = setInterval(() => void this.refresh(), 4000)
    return () => clearInterval(this.timer)
  }

  /** Syncs now and shows the result. `allowMassDelete` goes ahead with a sync that was stopped for deleting most
   * of the graph. */
  async now(allowMassDelete = false) {
    this.busy = true
    try {
      this.status = await this.backend.call('sync_now', { allow_mass_delete: allowMassDelete })
    } catch (e) {
      toast(errorMessage(e), 'error')
    } finally {
      this.busy = false
    }
  }
}
