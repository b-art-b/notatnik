// What a highlight block says about its document, and how to open the document at it (PA-6, EA-7): used by the chip
// beside a `((id))` reference, by the highlights on a sidecar page, and by that page's _Open the document_.
import { openBook } from '../commands/epub'
import { openPdf } from '../commands/pdf'
import { isExternalDoc } from './remote'

export interface HighlightDoc {
  kind: 'pdf' | 'epub'
  /** The sidecar's `file`: a path of the graph, or the address of a document that is not in it. */
  file: string
  /** The page of a PDF highlight (0 for a book). */
  page: number
  /** What a chip says: `report.pdf p. 12`, `novel.epub · 3 · Calm`. */
  label: string
  /** The document's name. */
  name: string
}

interface Prop {
  key: string
  raw: string
}

/** The document a highlight block belongs to, from the `file` of its sidecar page and the block's own properties; `null`
 * for a block that is no highlight. A document that is not in the graph has an address for `file`, with no name or
 * extension to go by: the page's name has the name, and the highlight (a page or a CFI) tells the kind. */
export function highlightDoc(file: string | undefined, props: Prop[], pageName: string): HighlightDoc | null {
  if (!file) return null
  const get = (k: string) => props.find((p) => p.key === k)?.raw
  const pg = get('hl-page')
  const pdfPage = pg && Number.isInteger(Number(pg)) ? Number(pg) : null
  const isBook = props.some((p) => p.key === 'hl-cfi')
  const chapter = get('hl-chapter')
  if (isExternalDoc(file)) {
    const name = pageName.split('/').pop() ?? pageName
    if (pdfPage !== null) return { kind: 'pdf', file, page: pdfPage, label: `${name} p. ${pdfPage}`, name }
    if (isBook) return { kind: 'epub', file, page: 0, label: chapter ? `${name} · ${chapter}` : name, name }
    return null
  }
  const name = file.split('/').pop() ?? file
  if (file.toLowerCase().endsWith('.pdf')) {
    return pdfPage === null
      ? null
      : { kind: 'pdf', file, page: pdfPage, label: `${name} p. ${pdfPage}`, name }
  }
  if (file.toLowerCase().endsWith('.epub')) {
    return isBook
      ? { kind: 'epub', file, page: 0, label: chapter ? `${name} · ${chapter}` : name, name }
      : null
  }
  return null
}

/** The kind of the document of a sidecar page, from its file or, for an address, from its first highlight. */
export function sidecarKind(file: string | undefined, blocks: { props: Prop[] }[]): 'pdf' | 'epub' | null {
  if (!file) return null
  if (!isExternalDoc(file)) {
    const l = file.toLowerCase()
    return l.endsWith('.pdf') ? 'pdf' : l.endsWith('.epub') ? 'epub' : null
  }
  for (const b of blocks) {
    if (b.props.some((p) => p.key === 'hl-cfi')) return 'epub'
    if (b.props.some((p) => p.key === 'hl-page')) return 'pdf'
  }
  return null
}

/** Opens the document, at the highlight `uid` when there is one. */
export function openHighlight(doc: Pick<HighlightDoc, 'kind' | 'file' | 'page'>, uid?: string): void {
  if (doc.kind === 'pdf') void openPdf({ path: doc.file, page: doc.page || undefined, highlight: uid })
  else void openBook({ path: doc.file, highlight: uid })
}
