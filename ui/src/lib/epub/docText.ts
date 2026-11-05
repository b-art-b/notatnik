// The text of one chapter of a book as a string, with a way to go from a point of the page to an offset in the string
// and back to a range (EA-4). It is used to give a highlight its context and to find it again by its quote when its CFI
// no longer fits. Only the text nodes count; scripts and styles do not.
import { contextOf, quoteOf, resolveAnchor } from '../pdf/anchor'

export interface DocText {
  text: string
  /** The offset in `text` of a point in a text node (null when the node is not part of the text). */
  offsetOf(node: Node, offset: number): number | null
  /** A range over `text[from, to)`. */
  range(from: number, to: number): Range | null
}

export function docText(doc: Document): DocText {
  const nodes: { node: Text; start: number }[] = []
  let text = ''
  const walker = doc.createTreeWalker(doc.body ?? doc.documentElement, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) =>
      n.parentElement?.closest('script, style, noscript')
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT,
  })
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    nodes.push({ node: n as Text, start: text.length })
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
  return { quote: quoteOf(dt.text, a, b), context: contextOf(dt.text, a, b) }
}

/** Finds a stored highlight again in the chapter by its quote and context: the range, or null. */
export function refind(dt: DocText, h: { quote: string; context: string }): Range | null {
  const at = resolveAnchor(dt.text, { from: -1, to: -1, quote: h.quote, context: h.context })
  return at ? dt.range(at.from, at.to) : null
}
