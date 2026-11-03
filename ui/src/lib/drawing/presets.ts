// Pen presets of the drawing editor (DR-15): a few pens that are one tap away, each a colour, a width and an opacity
// (a highlighter is a wide pen with a low opacity). The pure part, apart from js-draw so it can be tested alone.

export interface Preset {
  /** `#rrggbb` */
  color: string
  /** js-draw's thickness, in canvas units */
  width: number
  /** 0..1 */
  alpha: number
}

/** Fine and thick black, red, blue and a yellow highlighter. */
export const DEFAULT_PRESETS: readonly Preset[] = [
  { color: '#000000', width: 2, alpha: 1 },
  { color: '#000000', width: 6, alpha: 1 },
  { color: '#d11f1f', width: 3, alpha: 1 },
  { color: '#2563eb', width: 3, alpha: 1 },
  { color: '#ffd400', width: 18, alpha: 0.4 },
]

export const PRESETS_KEY = 'notatnik.drawing.presets'

const hex2 = (v: number) =>
  Math.round(Math.min(1, Math.max(0, v)) * 255)
    .toString(16)
    .padStart(2, '0')

/** The preset a pen has right now (`r`, `g`, `b`, `a` are 0..1 as in js-draw's Color4). */
export function presetOf(c: { r: number; g: number; b: number; a: number }, width: number): Preset {
  return {
    color: `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}`,
    width: Math.round(width * 100) / 100,
    alpha: Math.round(c.a * 100) / 100,
  }
}

/** The same pen? (Widths and opacities are compared with a tolerance: they pass through floats.) */
export function samePen(a: Preset, b: Preset): boolean {
  return (
    a.color.toLowerCase() === b.color.toLowerCase() &&
    Math.abs(a.width - b.width) < 0.05 &&
    Math.abs(a.alpha - b.alpha) < 0.02
  )
}

/** What was stored, if it is a list of presets as long as the default one; null for anything else. */
export function validPresets(x: unknown): Preset[] | null {
  if (!Array.isArray(x) || x.length !== DEFAULT_PRESETS.length) return null
  const out: Preset[] = []
  for (const p of x) {
    if (
      !p ||
      typeof p.color !== 'string' ||
      !/^#[0-9a-fA-F]{6}$/.test(p.color) ||
      typeof p.width !== 'number' ||
      !(p.width > 0 && p.width < 1000) ||
      typeof p.alpha !== 'number' ||
      !(p.alpha > 0 && p.alpha <= 1)
    )
      return null
    out.push({ color: p.color, width: p.width, alpha: p.alpha })
  }
  return out
}

export function loadPresets(): Preset[] {
  try {
    const saved = validPresets(JSON.parse(localStorage.getItem(PRESETS_KEY) ?? 'null'))
    if (saved) return saved
  } catch {
    /* no storage, or text that is not JSON: the defaults */
  }
  return DEFAULT_PRESETS.map((p) => ({ ...p }))
}

export function savePresets(p: readonly Preset[]): void {
  try {
    localStorage.setItem(PRESETS_KEY, JSON.stringify(p))
  } catch {
    /* the choice is just not remembered */
  }
}

/** A human name for the tooltip: "Fine black pen", "Highlighter"... without pretending to know every colour. */
export function describePreset(p: Preset): string {
  const kind = p.alpha < 0.8 ? 'Highlighter' : 'Pen'
  return `${kind}, ${p.color}, width ${p.width}`
}
