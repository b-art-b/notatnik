import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { ApiError, GraphChangedEvent } from './bindings'
import { CommandError, type Transport } from './types'

/** Talks to the Rust side through the single `invoke_api` command (see src-tauri). */
export function createTauriTransport(): Transport {
  return {
    async call(name, args, slot) {
      try {
        return (await invoke('invoke_api', { name, args, slot: slot || null })) as never
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
    assetUrl: (path, slot) =>
      `asset://localhost/${encodeURIComponent(path.replace(/^(\.\.\/)+/, ''))}${slot ? `?slot=${slot}` : ''}`,
    openExternal: (url) => {
      void invoke('open_external', { url })
    },
    openAsset: (path, slot) => {
      void invoke('open_asset', {
        path: encodeURIComponent(path.replace(/^(\.\.\/)+/, '')),
        slot: slot || null,
      })
    },
    clipboardImage: async () => ((await invoke('clipboard_image')) as string | null) ?? null,
    pickFile: async () => ((await invoke('pick_file')) as string | null) ?? null,
    pickFolder: async () => ((await invoke('pick_folder')) as string | null) ?? null,
  }
}
