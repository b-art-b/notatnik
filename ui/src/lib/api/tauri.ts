import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { ApiError, GraphChangedEvent } from './bindings'
import { isMobile } from '../platform'
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
    // `asset://localhost/<path>` on Linux, macOS and iOS, `http://asset.localhost/<path>` on Android and Windows: the
    // platform's webview decides how it reaches a custom protocol, and Tauri's helper knows (it encodes the path once)
    assetUrl: (path, slot) =>
      `${convertFileSrc(path.replace(/^(\.\.\/)+/, ''), 'asset')}${slot ? `?slot=${slot}` : ''}`,
    openExternal: (url) => {
      void invoke('open_external', { url })
    },
    openAsset: (path, slot) => {
      void invoke('open_asset', {
        path: encodeURIComponent(path.replace(/^(\.\.\/)+/, '')),
        slot: slot || null,
      })
    },
    pickFile: async () => ((await invoke('pick_file')) as string | null) ?? null,
    // a phone has no folder chooser and no system clipboard to ask (the webview pastes a picture itself): without the
    // function the UI does not offer the button
    ...(isMobile
      ? {}
      : {
          clipboardImage: async () => ((await invoke('clipboard_image')) as string | null) ?? null,
          pickFolder: async () => ((await invoke('pick_folder')) as string | null) ?? null,
          saveExport: async (name: string, data: string) =>
            ((await invoke('save_export', { name, data })) as string | null) ?? null,
        }),
  }
}
