// The keys that go to a place of the app. One table feeds the key handler (`App.svelte`) and the hints on the
// sidebar entries, so the two cannot disagree. Alt+digit works everywhere, also while typing; the Logseq-style
// chord (`g` then a letter) only when no text field has the focus.
export type NavTarget = 'journals' | 'agenda' | 'pages' | 'search' | 'settings'

export const NAV_KEYS: Record<NavTarget, { digit: string; chord: string | null }> = {
  journals: { digit: '1', chord: 'j' },
  agenda: { digit: '2', chord: 'a' },
  pages: { digit: '3', chord: 'p' },
  search: { digit: '4', chord: null }, // `/` and Ctrl+Shift+F open it already
  settings: { digit: '5', chord: 's' },
}

/** `Alt+1`, the key shown next to the entry. */
export const navHint = (t: NavTarget): string => `Alt+${NAV_KEYS[t].digit}`

/** The tooltip: `Agenda (Alt+2, or g then a)`. */
export function navTitle(label: string, t: NavTarget): string {
  const chord = NAV_KEYS[t].chord
  return `${label} (${navHint(t)}${chord ? `, or g then ${chord}` : ''})`
}

/** The place Alt+digit goes to: the physical key (`code`), as Option+digit types another character on a Mac. */
export function navForAlt(
  e: Pick<KeyboardEvent, 'code' | 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>,
): NavTarget | null {
  if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return null
  const hit = (Object.keys(NAV_KEYS) as NavTarget[]).find((t) => e.code === `Digit${NAV_KEYS[t].digit}`)
  return hit ?? null
}

/** The place the letter after `g` goes to. */
export function navForChord(key: string): NavTarget | null {
  const k = key.toLowerCase()
  return (Object.keys(NAV_KEYS) as NavTarget[]).find((t) => NAV_KEYS[t].chord === k) ?? null
}
