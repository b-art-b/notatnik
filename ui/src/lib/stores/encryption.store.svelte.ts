import type { Backend, EncryptionStatus } from '../api'

/** What the backend reports about the encryption of one graph (the rules that need it are in `encryption.svelte.ts`). */
export class EncryptionStore {
  status = $state<EncryptionStatus | null>(null)

  constructor(private readonly backend: Backend) {}

  /** Reads the status now. */
  async refresh() {
    try {
      this.status = await this.backend.call('encryption_status', {})
    } catch {
      /* the graph was closed meanwhile */
    }
  }
}
