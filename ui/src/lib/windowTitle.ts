/** The desktop window's title bar does not follow `document.title`, so the same text is set on the native window. */
export async function setWindowTitle(title: string): Promise<void> {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setTitle(title)
  } catch {
    /* the browser tab title (document.title) is already set */
  }
}
