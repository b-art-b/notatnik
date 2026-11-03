import type { ApiError, Commands, GraphChangedEvent } from './bindings'

export type CommandName = keyof Commands

/** What the UI needs from "the backend", for the graph of one slot. */
export interface Backend {
  /** Which open graph this backend talks to: `''` is the first graph, `'1'` the second graph (MG-1). */
  readonly slot: string
  call<K extends CommandName>(name: K, args: Commands[K]['args']): Promise<Commands[K]['result']>
  /** Subscribes to `graph-changed` of this slot; returns an unsubscribe function. */
  onGraphChanged(cb: (e: GraphChangedEvent) => void): () => void
  /** URL for an image below the graph's `assets/` folder. */
  assetUrl(path: string): string
  /** Opens a URL in the system browser. */
  openExternal(url: string): void
  /** Opens a file that was copied into the graph (the path as a link writes it: `../assets/…`). */
  openAsset(path: string): void
  /** The system's folder chooser (the desktop app only): the folder picked, `null` when cancelled. Absent in the
   * browser, where the path has to be typed. */
  /** The picture on the system clipboard as base64 PNG (the desktop app only), for a paste that the webview
   * delivers without the image. `null` when there is none. */
  clipboardImage?: () => Promise<string | null>
  pickFolder?: () => Promise<string | null>
  /** The system's file chooser (the desktop app only): the file picked, `null` when cancelled. */
  pickFile?: () => Promise<string | null>
  /** The system's save dialog and the file written (the desktop app only; DR-22): `data` is the file as base64, the
   * result the path written or `null` when cancelled. Absent in the browser (the page downloads the file) and on a
   * phone or tablet. */
  saveExport?: (name: string, data: string) => Promise<string | null>
}

/** The way to the Rust side, for every slot. Two implementations: HTTP (tests, browser) and Tauri (desktop). */
export interface Transport {
  call<K extends CommandName>(
    name: K,
    args: Commands[K]['args'],
    slot: string,
  ): Promise<Commands[K]['result']>
  /** Subscribes to `graph-changed` of every slot (each event says its `slot`). */
  onGraphChanged(cb: (e: GraphChangedEvent) => void): () => void
  assetUrl(path: string, slot: string): string
  openExternal(url: string): void
  openAsset(path: string, slot: string): void
  clipboardImage?: () => Promise<string | null>
  pickFolder?: () => Promise<string | null>
  pickFile?: () => Promise<string | null>
  saveExport?: (name: string, data: string) => Promise<string | null>
}

/** The error every failed command rejects with. */
export class CommandError extends Error {
  readonly code: string
  constructor(e: ApiError) {
    super(e.message)
    this.code = e.code
  }
}
