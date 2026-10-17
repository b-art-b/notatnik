// Images that arrive in the editor by paste or drop (ST-8): which files of a clipboard or drag are images, and
// their bytes as base64 for the `save_asset` command.

const IMAGE_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp']

/** The image files of a paste or a drop, in order. Anything else (text, other files) is ignored. */
export function imageFiles(dt: DataTransfer | null): File[] {
  if (!dt) return []
  return Array.from(dt.files).filter((f) => IMAGE_TYPES.includes(f.type))
}

/** True while files are dragged over (the drop only happens when `dragover` was cancelled). */
export function hasFiles(dt: DataTransfer | null): boolean {
  return !!dt && Array.from(dt.types).includes('Files')
}

/** The file's bytes as base64 (without a `data:` prefix). */
export function toBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader()
    r.onload = () => resolve(String(r.result).replace(/^data:[^,]*,/, ''))
    r.onerror = () => reject(r.error ?? new Error('could not read the image'))
    r.readAsDataURL(file)
  })
}

/** A PNG file made from base64 (the clipboard picture the desktop shell reads). */
export function pngFromBase64(b64: string): File {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  return new File([bytes], 'image.png', { type: 'image/png' })
}

/** The name worth keeping in the stored file: a dropped file's own, not the generic one a clipboard gives. */
export function usefulName(f: File): string | null {
  return !f.name || /^image\.(png|jpe?g|gif|webp)$/i.test(f.name) ? null : f.name
}
