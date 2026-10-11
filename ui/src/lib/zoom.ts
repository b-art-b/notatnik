// Page zoom like Firefox: Ctrl +/-/0 and Ctrl+wheel. The level is a per-device convenience, so it lives in
// localStorage (never in the graph folder, ST-2).

/** Firefox's zoom steps. */
export const ZOOM_STEPS = [0.3, 0.5, 0.67, 0.8, 0.9, 1, 1.1, 1.2, 1.33, 1.5, 1.7, 2, 2.4, 3, 4, 5]
const KEY = 'notatnik.zoom'

/** The step after (or before) `current`, clamped at the ends. */
export function stepZoom(current: number, dir: 1 | -1): number {
  const i = ZOOM_STEPS.findIndex((z) => Math.abs(z - current) < 1e-6)
  if (i >= 0) return ZOOM_STEPS[Math.min(ZOOM_STEPS.length - 1, Math.max(0, i + dir))]
  // not on a step (hand-edited storage): move to the nearest step in that direction
  const next =
    dir > 0 ? ZOOM_STEPS.find((z) => z > current) : [...ZOOM_STEPS].reverse().find((z) => z < current)
  return next ?? (dir > 0 ? ZOOM_STEPS[ZOOM_STEPS.length - 1] : ZOOM_STEPS[0])
}

let level = 1

export function currentZoom(): number {
  return level
}

async function applyZoom(z: number): Promise<void> {
  // In the desktop app the webview zooms itself (true page zoom, layout viewport included).
  if ('__TAURI_INTERNALS__' in window) {
    try {
      const { getCurrentWebview } = await import('@tauri-apps/api/webview')
      await getCurrentWebview().setZoom(z)
      return
    } catch {
      /* fall through to CSS zoom */
    }
  }
  document.documentElement.style.zoom = z === 1 ? '' : String(z)
}

export function setZoom(z: number): void {
  level = z
  void applyZoom(z)
  try {
    localStorage.setItem(KEY, String(z))
  } catch {
    /* storage unavailable: the zoom just isn't remembered */
  }
}

export function initZoom(): void {
  try {
    const v = Number(localStorage.getItem(KEY))
    if (v >= ZOOM_STEPS[0] && v <= ZOOM_STEPS[ZOOM_STEPS.length - 1]) setZoom(v)
  } catch {
    /* ignore */
  }
}

/** Handles Ctrl/Cmd +, -, 0. Returns true when the key was a zoom key. */
export function zoomKey(e: KeyboardEvent): boolean {
  if (!(e.ctrlKey || e.metaKey) || e.altKey) return false
  if (e.key === '+' || e.key === '=') setZoom(stepZoom(level, 1))
  else if (e.key === '-' || e.key === '_') setZoom(stepZoom(level, -1))
  else if (e.key === '0') setZoom(1)
  else return false
  e.preventDefault()
  return true
}

/** Ctrl+wheel zooms, as in Firefox. */
export function zoomWheel(e: WheelEvent): void {
  if (!(e.ctrlKey || e.metaKey) || e.deltaY === 0) return
  e.preventDefault()
  setZoom(stepZoom(level, e.deltaY < 0 ? 1 : -1))
}
