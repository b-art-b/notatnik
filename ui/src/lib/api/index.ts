import { createHttpBackend } from './http'
import { createTauriBackend } from './tauri'
import type { Backend } from './types'

export type { Backend } from './types'
export { CommandError } from './types'
export type * from './bindings'

/** Chosen at build time: `VITE_BACKEND=tauri` for the desktop app, otherwise the HTTP dev backend. */
export const backend: Backend = import.meta.env.VITE_BACKEND === 'tauri' ? createTauriBackend() : createHttpBackend()
