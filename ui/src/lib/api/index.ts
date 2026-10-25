import { createHttpTransport } from './http'
import { createTauriTransport } from './tauri'
import type { Backend, Transport } from './types'

export type { Backend, Transport } from './types'
export { CommandError } from './types'
export type * from './bindings'

declare global {
  interface Window {
    /** The window's way to the Rust side, shared with the panes in frames (MG-6). */
    __notatnik?: Transport
  }
}

/**
 * The slot this page shows: `''` is the first graph. A second graph is shown in a frame of the same origin whose
 * address says `?slot=1`; the frame has its own copy of every store, so each pane is a whole app of its own.
 */
function slotOfThisPage(): string {
  if (window.parent === window) return ''
  return new URLSearchParams(window.location.search).get('slot') ?? ''
}

/** Chosen at build time: `VITE_BACKEND=tauri` for the desktop app, otherwise the HTTP dev backend. */
function transportOfThisWindow(): Transport {
  if (window.parent !== window) {
    try {
      if (window.parent.__notatnik) return window.parent.__notatnik
    } catch {
      /* a parent of another origin is none of ours; fall through to a transport of our own */
    }
  }
  window.__notatnik ??=
    import.meta.env.VITE_BACKEND === 'tauri' ? createTauriTransport() : createHttpTransport()
  return window.__notatnik
}

/** A backend for one slot: the transport with the slot added to every call and the other slots' events left out. */
export function createBackend(transport: Transport, slot: string): Backend {
  return {
    slot,
    call: (name, args) => transport.call(name, args, slot),
    onGraphChanged: (cb) => transport.onGraphChanged((e) => (e.slot ?? '') === slot && cb(e)),
    assetUrl: (path) => transport.assetUrl(path, slot),
    openExternal: (url) => transport.openExternal(url),
    openAsset: (path) => transport.openAsset(path, slot),
    clipboardImage: transport.clipboardImage,
    pickFile: transport.pickFile,
    pickFolder: transport.pickFolder,
  }
}

export const backend: Backend = createBackend(transportOfThisWindow(), slotOfThisPage())

/** The backend of the window itself (the first graph), whatever pane this page is: for calls that look at all graphs. */
export const workspace: Backend = createBackend(transportOfThisWindow(), '')
