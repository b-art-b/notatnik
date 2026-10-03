import type { ApiError, Commands, GraphChangedEvent } from './bindings'

export type CommandName = keyof Commands

/** What the UI needs from "the backend". Two implementations: HTTP (tests, browser) and Tauri (desktop). */
export interface Backend {
  call<K extends CommandName>(name: K, args: Commands[K]['args']): Promise<Commands[K]['result']>
  /** Subscribes to `graph-changed`; returns an unsubscribe function. */
  onGraphChanged(cb: (e: GraphChangedEvent) => void): () => void
  /** URL for an image below the graph's `assets/` folder. */
  assetUrl(path: string): string
  /** Opens a URL in the system browser. */
  openExternal(url: string): void
}

/** The error every failed command rejects with. */
export class CommandError extends Error {
  readonly code: string
  constructor(e: ApiError) {
    super(e.message)
    this.code = e.code
  }
}
