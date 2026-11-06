// Documents beside the notes (DD-1..DD-8): a PDF or an EPUB can be docked into the right side panel instead of covering the
// notes. Full screen is the default; docking is a button in the viewer and a setting that is off.
import { app } from './app.svelte'

/** The window must be at least this wide to dock: the notes keep 320 px and the panel 280 px, with room to spare. */
export const DOCK_MIN_WIDTH = 900
const SETTING_KEY = 'notatnik.dockDocuments'

const win = $state({ width: typeof window === 'undefined' ? 1920 : window.innerWidth })
if (typeof window !== 'undefined') window.addEventListener('resize', () => (win.width = window.innerWidth))

/** The window is wide enough to dock a document. */
export const canDock = (): boolean => win.width >= DOCK_MIN_WIDTH

let memory = false

/** Whether documents open docked (this device; off by default). */
export function dockSetting(): boolean {
  try {
    return localStorage.getItem(SETTING_KEY) === 'yes'
  } catch {
    return memory
  }
}

export function setDockSetting(on: boolean): void {
  memory = on
  try {
    localStorage.setItem(SETTING_KEY, on ? 'yes' : 'no')
  } catch {
    // a private window may refuse; the choice then lasts until the page is closed
  }
}

/** A document is docked in the side panel. */
export const dockedNow = (): boolean => !!(app.pdf?.docked || app.book?.docked)

/** Whether a document that opens now is docked: one that is docked already is replaced in the panel whatever the setting
 * says (DD-8); otherwise the setting decides, on a window that is wide enough. */
export const opensDocked = (): boolean => dockedNow() || (dockSetting() && canDock())

/** The document tab of the side panel is shown and the panel is open (it opens if it was closed). */
export function showDocumentTab(): void {
  if (app.rightTab !== 'document') {
    app.rightTabBefore = app.rightTab === 'pages' || app.rightTab === 'tasks' ? app.rightTab : 'refs'
    app.rightTab = 'document'
  }
  app.rightOpen = true
  app.rightFocus = false
}

/** A document was closed or brought back to full screen: the tab that was open before comes back. */
export function leaveDocumentTab(): void {
  if (app.rightTab === 'document') app.rightTab = app.rightTabBefore
  app.dockRect = null
}

/** Moves the open document into the side panel (`on`) or back to full screen. */
export function dockDocument(on: boolean): void {
  const open = app.pdf ?? app.book
  if (!open) return
  if (on && !canDock()) return
  open.docked = on
  if (on) showDocumentTab()
  else leaveDocumentTab()
}
