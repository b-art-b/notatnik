// Device preferences of the drawing editor that are not part of a drawing: whether pens snap to shapes when held
// (DR-18) and where the toolbar is (DR-21). In localStorage, per device, like the pens (DR-4).

export interface DrawingPrefs {
  /** Hold the pen still at the end of a stroke and it becomes a straight line or a clean shape (off: DR-3). */
  snapOnHold: boolean
  /** The toolbar below the canvas instead of above it (a tablet held in the hands). */
  toolbarBottom: boolean
}

const KEY = 'notatnik.drawing.prefs'

export function loadPrefs(): DrawingPrefs {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<DrawingPrefs>
    return { snapOnHold: v.snapOnHold === true, toolbarBottom: v.toolbarBottom === true }
  } catch {
    return { snapOnHold: false, toolbarBottom: false }
  }
}

export function savePrefs(p: DrawingPrefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(p))
  } catch {
    /* the choice is just not remembered */
  }
}
