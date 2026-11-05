// A highlight's place in the text of a page (PA-3): the quote, the numbers that say where it is, and finding it again
// when the numbers no longer fit. Pure functions over strings, no DOM and no pdf.js.

/** How much text before and after the quote is kept to tell equal quotes apart. */
export const CONTEXT_CHARS = 40

/** Runs of white space (also the newline between lines) become one space; nothing at the ends. */
export function squash(s: string): string {
  return s.replace(/\s+/g, ' ').trim()
}

/** The quote of `page[from, to)` as it is written in the block. */
export function quoteOf(page: string, from: number, to: number): string {
  return squash(page.slice(from, to))
}

// `|` separates the two halves in the file, so it must not occur inside them
const clean = (s: string) => squash(s).replace(/\|/g, '¦')

/** `before | after`: up to `CONTEXT_CHARS` characters on each side of the quote. */
export function contextOf(page: string, from: number, to: number): string {
  return `${clean(page.slice(Math.max(0, from - CONTEXT_CHARS), from))} | ${clean(page.slice(to, to + CONTEXT_CHARS))}`
}

export function splitContext(context: string): { before: string; after: string } {
  const i = context.indexOf('|')
  if (i < 0) return { before: '', after: '' }
  // the property value is trimmed when it is read, so an empty half leaves no space beside the bar
  return { before: context.slice(0, i).trim(), after: context.slice(i + 1).trim() }
}

/** The page's text without white space, and for each of its characters the offset in the page. */
function stripped(page: string): { text: string; at: number[] } {
  let text = ''
  const at: number[] = []
  for (let i = 0; i < page.length; i++) {
    if (/\s/.test(page[i])) continue
    text += page[i]
    at.push(i)
  }
  return { text, at }
}

const strip = (s: string) => s.replace(/\s+/g, '')

/** How well the text around `[from, to)` agrees with the stored context: 0 to 2 (one point for each side). */
function contextScore(page: string, from: number, to: number, context: string): number {
  const { before, after } = splitContext(context)
  let score = 0
  const b = strip(before)
  const a = strip(after)
  if (b && strip(page.slice(Math.max(0, from - CONTEXT_CHARS * 2), from)).endsWith(b)) score++
  if (a && strip(page.slice(to, to + CONTEXT_CHARS * 2)).startsWith(a)) score++
  return score
}

export interface Stored {
  from: number
  to: number
  /** The text of the block (what the person sees; it may have been edited). */
  quote: string
  context: string
}

/**
 * Where a stored highlight is in the text of the page now: the numbers when the text at them is the quote (white space
 * ignored); otherwise the place where the quote is on that page (the one the context fits best, then the closest to
 * the old numbers) and the numbers are corrected by the caller; otherwise the numbers if the context still fits them
 * (the quote was edited by hand); otherwise null (not found).
 */
export function resolveAnchor(page: string, h: Stored): { from: number; to: number } | null {
  const want = strip(h.quote)
  if (h.from >= 0 && h.to > h.from && h.to <= page.length) {
    if (want && strip(page.slice(h.from, h.to)) === want) return { from: h.from, to: h.to }
  }
  if (want) {
    const s = stripped(page)
    const hits: { from: number; to: number }[] = []
    for (let i = s.text.indexOf(want); i >= 0; i = s.text.indexOf(want, i + 1)) {
      hits.push({ from: s.at[i], to: s.at[i + want.length - 1] + 1 })
    }
    if (hits.length > 0) {
      let best = hits[0]
      let bestKey = [-1, 0]
      for (const c of hits) {
        const key = [contextScore(page, c.from, c.to, h.context), -Math.abs(c.from - h.from)]
        if (key[0] > bestKey[0] || (key[0] === bestKey[0] && key[1] > bestKey[1])) {
          best = c
          bestKey = key
        }
      }
      return best
    }
  }
  if (
    h.from >= 0 &&
    h.to > h.from &&
    h.to <= page.length &&
    contextScore(page, h.from, h.to, h.context) === 2
  ) {
    return { from: h.from, to: h.to }
  }
  return null
}
