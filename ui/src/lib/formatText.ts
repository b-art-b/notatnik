// Inline formatting of selected text (FK-1..FK-6): the Markdown markers around a selection, added or removed.
// Pure: text in, text and selection out, so the corner cases are unit tests and the editor only applies the result.

export type Marker = '**' | '*' | '~~' | '==' | '`'

export interface Wrapped {
  text: string
  /** The selection afterwards: the same words, without the markers' characters. */
  from: number
  to: number
}

/** How many markers of kind `ch` (a single character) there are in a row, ending at `end` going backwards. */
function runBefore(text: string, end: number, ch: string): number {
  let n = 0
  while (end - n > 0 && text[end - n - 1] === ch) n++
  return n
}
function runAfter(text: string, start: number, ch: string): number {
  let n = 0
  while (start + n < text.length && text[start + n] === ch) n++
  return n
}

/** Does the run of `k` asterisks around a word include this marker? `*` is italic (an odd count: `*a*`, `***a***`),
 * `**` is bold (two or more: `**a**`, `***a***`). So `**a**` is not italic, and `***a***` is both. */
function starsHave(marker: '*' | '**', k: number): boolean {
  return marker === '*' ? k % 2 === 1 : k >= 2
}

/** The part of `[a, b)` without leading and trailing spaces; null when nothing is left. */
function trimmed(text: string, a: number, b: number): [number, number] | null {
  while (a < b && /\s/.test(text[a])) a++
  while (b > a && /\s/.test(text[b - 1])) b--
  return a < b ? [a, b] : null
}

type Plan = { kind: 'wrap' } | { kind: 'unwrap'; outside: boolean; remove: number }

/** What the marker is doing at `[a, b)`: markers just outside, markers inside the selection, or none. */
function planFor(text: string, a: number, b: number, marker: Marker): Plan {
  const len = marker.length
  if (marker === '*' || marker === '**') {
    const outside = Math.min(runBefore(text, a, '*'), runAfter(text, b, '*'))
    if (starsHave(marker, outside)) return { kind: 'unwrap', outside: true, remove: len }
    // the markers are part of the selection: `**a**` selected whole; the word between must be left
    let inside = Math.min(runAfter(text, a, '*'), runBefore(text, b, '*'))
    while (inside > 0 && 2 * inside >= b - a) inside--
    if (starsHave(marker, inside)) return { kind: 'unwrap', outside: false, remove: len }
    return { kind: 'wrap' }
  }
  if (text.slice(a - len, a) === marker && text.slice(b, b + len) === marker && a >= len)
    return { kind: 'unwrap', outside: true, remove: len }
  if (b - a > 2 * len && text.slice(a, a + len) === marker && text.slice(b - len, b) === marker)
    return { kind: 'unwrap', outside: false, remove: len }
  return { kind: 'wrap' }
}

/**
 * Wraps the text of `[from, to)` in `marker`, or takes the marker off when it is already there. A selection over
 * several lines is done line by line (a marker never spans a line break), spaces at the ends of a line stay outside
 * the markers. Null when there is nothing to do (an empty selection, only spaces).
 */
export function toggleWrap(text: string, from: number, to: number, marker: Marker): Wrapped | null {
  if (from >= to) return null
  // the pieces: each non-empty line of the selection, without its end spaces
  const pieces: [number, number][] = []
  let lineStart = from
  while (lineStart <= to) {
    let lineEnd = text.indexOf('\n', lineStart)
    if (lineEnd < 0 || lineEnd > to) lineEnd = to
    const t = trimmed(text, lineStart, lineEnd)
    if (t) pieces.push(t)
    lineStart = lineEnd + 1
  }
  if (pieces.length === 0) return null
  const plans = pieces.map(([a, b]) => planFor(text, a, b, marker))
  // all marked already: take the markers off; otherwise put them on the lines that have none
  const unwrapAll = plans.every((p) => p.kind === 'unwrap')
  let out = ''
  let pos = 0
  let selFrom = -1
  let selTo = -1
  pieces.forEach(([a, b], i) => {
    const plan = plans[i]
    const len = marker.length
    out += text.slice(pos, a)
    pos = b
    if (unwrapAll && plan.kind === 'unwrap') {
      if (plan.outside) {
        // the markers are outside [a, b): they are already in `out` (the end of it) and in the text after b
        out = out.slice(0, out.length - plan.remove)
        pos = b + plan.remove
        if (selFrom < 0) selFrom = out.length
        out += text.slice(a, b)
      } else {
        if (selFrom < 0) selFrom = out.length
        out += text.slice(a + plan.remove, b - plan.remove)
      }
    } else if (plan.kind === 'wrap') {
      out += marker
      if (selFrom < 0) selFrom = out.length
      out += text.slice(a, b) + marker
    } else {
      // a line that has the markers already, while another has not: left as it is
      if (selFrom < 0) selFrom = out.length
      out += text.slice(a, b)
    }
    selTo = out.length - (!unwrapAll && plan.kind === 'wrap' ? len : 0)
  })
  out += text.slice(pos)
  return { text: out, from: selFrom, to: selTo }
}
