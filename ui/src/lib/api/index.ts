import { createHttpTransport } from './http'
import { createTauriTransport } from './tauri'
import type { Backend, Transport } from './types'

export type { Backend, Transport } from './types'
export { CommandError } from './types'
export type * from './bindings'

/** Chosen at build time: `VITE_BACKEND=tauri` for the desktop app, otherwise the HTTP dev backend. */
const transport: Transport =
  import.meta.env.VITE_BACKEND === 'tauri' ? createTauriTransport() : createHttpTransport()

/** A backend for one slot: the transport with the slot added to every call and the other slots' events left out. */
export function backendFor(slot: string): Backend {
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
    saveExport: transport.saveExport,
  }
}

let activeSlot = ''

/** Makes the graph in `slot` the one `backend` talks to (the page area shows that graph). */
export function setActiveSlot(slot: string) {
  activeSlot = slot
}

/**
 * The backend of the *active* graph: the one whose page the page area shows. Everything in the page area uses this one;
 * a sidebar section talks to its own graph with `backendFor(slot)` (`GraphState.backend`).
 */
export const backend: Backend = {
  get slot() {
    return activeSlot
  },
  call: (name, args) => transport.call(name, args, activeSlot),
  onGraphChanged: (cb) => transport.onGraphChanged((e) => (e.slot ?? '') === activeSlot && cb(e)),
  assetUrl: (path) => transport.assetUrl(path, activeSlot),
  openExternal: (url) => transport.openExternal(url),
  openAsset: (path) => transport.openAsset(path, activeSlot),
  clipboardImage: transport.clipboardImage,
  pickFile: transport.pickFile,
  pickFolder: transport.pickFolder,
  saveExport: transport.saveExport,
}

/** Calls that look at all graphs (`slots`, `close_slot`, the transfer) go to the first graph's state, whichever is active. */
export const workspace: Backend = backendFor('')
