// How the children of a block are marked (LS-1..LS-3): the hidden block property `list::` on the parent.
import type { BlockView } from './api'

export type ListStyle = 'bullets' | 'numbered' | 'letters' | 'roman'

/** The choices of the menu and the palette: the value written to `list::` (none for bullets) and the words. */
export const LIST_STYLES: { style: ListStyle; label: string }[] = [
  { style: 'bullets', label: 'Bullets' },
  { style: 'numbered', label: 'Numbered' },
  { style: 'letters', label: 'Letters' },
  { style: 'roman', label: 'Roman numerals' },
]

/** The style a block gives its children; anything but the three known values is the bullet dot. */
export function listStyleOf(block: BlockView): ListStyle {
  const v = block.props
    .find((p) => p.key === 'list')
    ?.raw.trim()
    .toLowerCase()
  return v === 'numbered' || v === 'letters' || v === 'roman' ? v : 'bullets'
}

/** 1 → a, 26 → z, 27 → aa, 28 → ab ... */
function letters(n: number): string {
  let s = ''
  for (let k = n; k > 0; k = Math.floor((k - 1) / 26)) s = String.fromCharCode(97 + ((k - 1) % 26)) + s
  return s
}

const ROMAN: [number, string][] = [
  [1000, 'm'],
  [900, 'cm'],
  [500, 'd'],
  [400, 'cd'],
  [100, 'c'],
  [90, 'xc'],
  [50, 'l'],
  [40, 'xl'],
  [10, 'x'],
  [9, 'ix'],
  [5, 'v'],
  [4, 'iv'],
  [1, 'i'],
]

function roman(n: number): string {
  if (n >= 4000) return String(n) // roman numerals do not reach it; show the number rather than a long row of m
  let s = ''
  for (const [v, r] of ROMAN)
    while (n >= v) {
      s += r
      n -= v
    }
  return s
}

/** The mark of the child at `index` (0-based) among its siblings: `1.`, `a.`, `i.`; null for the bullet dot. */
export function listMark(style: ListStyle, index: number): string | null {
  const n = index + 1
  switch (style) {
    case 'numbered':
      return `${n}.`
    case 'letters':
      return `${letters(n)}.`
    case 'roman':
      return `${roman(n)}.`
    default:
      return null
  }
}
