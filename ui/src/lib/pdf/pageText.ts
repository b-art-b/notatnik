// The text of one PDF page as the highlights count it (PA-3, `docs/phases/pdf-annotation.md`).
//
// A highlight stores only where its quote starts and ends: two character offsets into this text. Writing a highlight
// and reading it back both go through `buildPageText`, so the numbers mean the same thing every time. Do not change
// this function without a migration: every stored `hl-from` / `hl-to` depends on it.

/** The part of a pdf.js text item that matters here. Marked-content entries of pdf.js have no `str` and are skipped. */
export interface TextItem {
  str?: string
  hasEOL?: boolean
}

export interface PageText {
  /** The items' `str` joined in order, with a newline after an item that ends a line. */
  text: string
  /** Where each text item (the ones with a `str`, in order) starts in `text`; the same index as the text layer's span. */
  starts: number[]
}

export function buildPageText(items: readonly object[]): PageText {
  let text = ''
  const starts: number[] = []
  for (const it of items as readonly TextItem[]) {
    if (typeof it.str !== 'string') continue
    starts.push(text.length)
    text += it.str
    if (it.hasEOL) text += '\n'
  }
  return { text, starts }
}

/** The item that holds `offset` and the offset inside it. An offset in the newline after an item counts as the end
 * of that item; one past the end of the text is the end of the last item. */
export function locate(
  pt: PageText,
  strLengths: readonly number[],
  offset: number,
): { item: number; at: number } | null {
  const n = pt.starts.length
  if (n === 0) return null
  // last item that starts at or before the offset
  let lo = 0
  let hi = n - 1
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    if (pt.starts[mid] <= offset) lo = mid
    else hi = mid - 1
  }
  return { item: lo, at: Math.max(0, Math.min(strLengths[lo], offset - pt.starts[lo])) }
}
