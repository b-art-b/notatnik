// The text of one chapter of a book as a string, with a way to go from a point of the page to an offset in the string
// and back to a range (EA-4). It is used to give a highlight its context and to find it again by its quote when its CFI
// no longer fits. Only the text nodes count; scripts and styles do not.
import { contextOf, resolveAnchor, squash } from '../pdf/anchor'

export interface DocText {
  text: string
  /** The offset in `text` of a point in a text node (null when the node is not part of the text). */
  offsetOf(node: Node, offset: number): number | null
  /** A range over `text[from, to)`. */
  range(from: number, to: number): Range | null
  /** `text[from, to)` as paragraphs: one line for each block of the book (a paragraph, a heading, a list item, a table
   * cell) and for each line break, white space inside a line made one space, empty lines left out. `text` itself has no
   * breaks (its offsets are what the stored highlights are found by), so the breaks come from the book's own markup. */
  paragraphs(from: number, to: number): string
}

/** The elements that start a new line of text in a book. */
const BLOCKS =
  'p, div, li, h1, h2, h3, h4, h5, h6, blockquote, pre, tr, td, th, dt, dd, section, article, aside, figure, figcaption, ul, ol, table, body'

export function docText(doc: Document): DocText {
  const nodes: { node: Text; start: number; block: Element | null }[] = []
  // offsets in `text` where the book has a line break (`<br>`)
  const breaks: number[] = []
  let text = ''
  const walker = doc.createTreeWalker(
    doc.body ?? doc.documentElement,
    NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT,
    {
      acceptNode: (n) => {
        const el = n.nodeType === Node.ELEMENT_NODE ? (n as Element) : n.parentElement
        if (el?.closest('script, style, noscript')) return NodeFilter.FILTER_REJECT
        // an element is only looked at to find `<br>`; its text nodes are visited on their own
        return NodeFilter.FILTER_ACCEPT
      },
    },
  )
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (n.nodeType === Node.ELEMENT_NODE) {
      if ((n as Element).localName === 'br') breaks.push(text.length)
      continue
    }
    nodes.push({
      node: n as Text,
      start: text.length,
      block: (n as Text).parentElement?.closest(BLOCKS) ?? null,
    })
    text += (n as Text).data
  }
  const find = (offset: number) => {
    let lo = 0
    let hi = nodes.length - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (nodes[mid].start <= offset) lo = mid
      else hi = mid - 1
    }
    return nodes[lo]
  }
  return {
    text,
    offsetOf(node, offset) {
      if (node.nodeType === Node.TEXT_NODE) {
        const hit = nodes.find((x) => x.node === node)
        return hit ? hit.start + offset : null
      }
      // a point between elements: before the child at `offset`
      const child = node.childNodes[offset]
      const first = child ? firstText(child) : null
      if (first) return nodes.find((x) => x.node === first)?.start ?? null
      const last = nodes.length ? nodes[nodes.length - 1] : null
      return last ? last.start + last.node.data.length : null
    },
    paragraphs(from, to) {
      const lines: string[] = ['']
      let last: Element | null | undefined
      for (const x of nodes) {
        const end = x.start + x.node.data.length
        if (end <= from || x.start >= to) continue
        const piece = x.node.data.slice(Math.max(0, from - x.start), Math.max(0, to - x.start))
        // a new block, or a `<br>` before this piece, is a new line; a newline inside a text node is only white space
        if (last !== undefined && (x.block !== last || (x.start > from && breaks.includes(x.start))))
          lines.push('')
        lines[lines.length - 1] += piece
        last = x.block
      }
      return lines.map(squash).filter(Boolean).join('\n')
    },
    range(from, to) {
      if (nodes.length === 0 || to <= from) return null
      const a = find(Math.max(0, from))
      const b = find(Math.max(0, to - 1))
      const r = doc.createRange()
      r.setStart(a.node, Math.min(from - a.start, a.node.data.length))
      r.setEnd(b.node, Math.min(to - b.start, b.node.data.length))
      return r
    },
  }
}

function firstText(n: Node): Text | null {
  if (n.nodeType === Node.TEXT_NODE) return n as Text
  for (const c of Array.from(n.childNodes)) {
    const t = firstText(c)
    if (t) return t
  }
  return null
}

/** The quote and the context of a range, as they are stored in the block of a highlight. */
export function describeRange(dt: DocText, range: Range): { quote: string; context: string } | null {
  const a = dt.offsetOf(range.startContainer, range.startOffset)
  const b = dt.offsetOf(range.endContainer, range.endOffset)
  if (a == null || b == null || b <= a) return null
  return { quote: dt.paragraphs(a, b), context: contextOf(dt.text, a, b) }
}

/** Finds a stored highlight again in the chapter by its quote and context: the range, or null. */
export function refind(dt: DocText, h: { quote: string; context: string }): Range | null {
  const at = resolveAnchor(dt.text, { from: -1, to: -1, quote: h.quote, context: h.context })
  return at ? dt.range(at.from, at.to) : null
}
