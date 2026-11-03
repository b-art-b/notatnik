// A drawing outside the app (DR-22): as a PNG on the clipboard, or as a PNG or an SVG file. The functions that need
// js-draw take the editor; the sizes and names are pure and tested alone.
import type Editor from 'js-draw'
import { Vec2 } from 'js-draw'
import { backend } from '../api'

/** The longest side of an exported PNG, in pixels: sharp enough to read handwriting, small enough to paste anywhere. */
const TARGET_SIDE = 2000
const MAX_SIDE = 4096

/** The pixel size of the PNG of a drawing `w` × `h` canvas units: scaled so the longer side is about 2000 px, never
 * beyond 4096, never an empty image. */
export function pngSize(w: number, h: number): { w: number; h: number } {
  const longer = Math.max(w, h, 1)
  const k = Math.min(TARGET_SIDE / longer, MAX_SIDE / longer)
  return { w: Math.max(1, Math.round(w * k)), h: Math.max(1, Math.round(h * k)) }
}

/** A file name for the drawing: its name when it has one, else `drawing`; nothing a file system would refuse. */
export function exportName(name: string, ext: 'png' | 'svg'): string {
  const base = name
    .trim()
    // eslint-disable-next-line no-control-regex
    .replace(/[\\/:*?"<>|\u0000-\u001f]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 80)
  return `${base || 'drawing'}.${ext}`
}

/** The bytes of a `data:` URL as a Blob. (`fetch` of a data URL is refused by the page's content security policy.) */
export function dataUrlToBlob(url: string): Blob {
  const m = /^data:([^;,]*)(;base64)?,(.*)$/s.exec(url)
  if (!m) throw new Error('not a data URL')
  const text = m[2] ? atob(m[3]) : decodeURIComponent(m[3])
  const bytes = new Uint8Array(text.length)
  for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i)
  return new Blob([bytes], { type: m[1] || 'application/octet-stream' })
}

export function pngBlob(ed: Editor): Promise<Blob> {
  const r = ed.getImportExportRect()
  const s = pngSize(r.width, r.height)
  return Promise.resolve(dataUrlToBlob(ed.toDataURL('image/png', Vec2.of(s.w, s.h))))
}

export function svgBlob(ed: Editor): Blob {
  return new Blob([new XMLSerializer().serializeToString(ed.toSVG())], { type: 'image/svg+xml' })
}

/** The picture on the clipboard. false: this webview cannot put a picture there (use Save as PNG). */
export async function copyPng(ed: Editor): Promise<boolean> {
  if (typeof ClipboardItem === 'undefined' || !navigator.clipboard?.write) return false
  try {
    // Safari wants the promise given to the item, inside the click, not an awaited blob
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': pngBlob(ed) })])
    return true
  } catch {
    return false
  }
}

async function base64(blob: Blob): Promise<string> {
  const bytes = new Uint8Array(await blob.arrayBuffer())
  let s = ''
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(s)
}

/** Saves the file: the system's save dialog in the desktop app, a download in a browser. Resolves with where it went
 * (a path, or `download`), or null when the person cancelled the dialog. */
export async function saveFile(blob: Blob, fileName: string): Promise<string | null> {
  if (backend.saveExport) return await backend.saveExport(fileName, await base64(blob))
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = fileName
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
  return 'download'
}
