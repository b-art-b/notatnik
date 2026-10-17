import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { ApiError, GraphChangedEvent } from './bindings'
import { type Backend, CommandError } from './types'

/** Talks to the Rust side through the single `invoke_api` command (see src-tauri). */
export function createTauriBackend(): Backend {
  return {
    async call(name, args) {
      try {
        return (await invoke('invoke_api', { name, args })) as never
      } catch (e) {
        throw new CommandError(e as ApiError)
      }
    },
    onGraphChanged(cb) {
      let off: (() => void) | undefined
      let cancelled = false
      void listen<GraphChangedEvent>('graph-changed', (ev) => cb(ev.payload)).then((u) => {
        if (cancelled) u()
        else off = u
      })
      return () => {
        cancelled = true
        off?.()
      }
    },
    assetUrl: (path) => `asset://localhost/${encodeURIComponent(path.replace(/^(\.\.\/)+/, ''))}`,
    openExternal: (url) => {
      void invoke('open_external', { url })
    },
    clipboardImage: async () => ((await invoke('clipboard_image')) as string | null) ?? null,
    pickFolder: async () => ((await invoke('pick_folder')) as string | null) ?? null,
  }
}
