// The only file that imports foliate-js (`ui/vendor/foliate-js`, see its README). The library is not stable, so what the
// app uses of it is written down here as types, and a new version of it is a change to this file and the vendored copy
// and to nothing else. It is loaded the first time a book is opened.

/** An entry of the table of contents. */
export interface TocItem {
  label: string
  href: string
  subitems?: TocItem[]
}

export interface Location {
  /** 0 to 1 through the whole book. */
  fraction: number
  /** The CFI of the start of what is on the screen. */
  cfi: string
  /** The table-of-contents entry the reader is in. */
  tocItem?: { label: string; href: string }
  /** The section (chapter file) the reader is in. */
  index: number
  range: Range
}

export interface SearchExcerpt {
  pre: string
  match: string
  post: string
}
export interface SearchHit {
  cfi: string
  excerpt: SearchExcerpt
}
export type SearchEvent = 'done' | { progress: number } | { label: string; subitems: SearchHit[] }

export interface Book {
  metadata?: { title?: string | Record<string, string> }
  toc?: TocItem[]
  sections: { linear?: string }[]
  rendition?: { layout?: string }
  dir?: string
}

type Draw = (rects: DOMRect[], options?: Record<string, unknown>) => SVGElement

/** The `<foliate-view>` element, as far as the app uses it. */
export interface FoliateView extends HTMLElement {
  book: Book
  renderer: HTMLElement & {
    setStyles(css: string): void
    getContents(): { index: number; doc: Document }[]
  }
  lastLocation: Location | null
  open(book: Book): Promise<void>
  close(): void
  init(opts: { lastLocation?: string | null; showTextStart?: boolean }): Promise<void>
  goTo(target: string | number): Promise<{ index: number } | undefined>
  goToFraction(f: number): Promise<void>
  next(): Promise<void>
  prev(): Promise<void>
  goLeft(): Promise<void>
  goRight(): Promise<void>
  getCFI(index: number, range?: Range): string
  resolveCFI(cfi: string): { index: number; anchor: (doc: Document) => Range }
  getProgressOf(index: number, range: Range): { tocItem?: { label: string; href: string } }
  addAnnotation(a: { value: string }, remove?: boolean): Promise<unknown>
  deleteAnnotation(a: { value: string }): Promise<unknown>
  search(opts: { query: string; index?: number | null }): AsyncGenerator<SearchEvent>
  clearSearch(): void
}

export type Highlighter = Draw

export interface Foliate {
  /** Reads the book out of the file; throws for a file that is not a book. */
  makeBook(file: File): Promise<Book>
  /** The functions that draw a mark over a range. */
  highlight: Draw
  outline: Draw
}

let loaded: Promise<Foliate> | null = null

// `import.meta.glob` lets Vite bundle the vendored JavaScript without TypeScript following the import into it (the
// library is not written for our compiler settings). What is used of it is declared above.
const modules = import.meta.glob<unknown>([
  '../../../vendor/foliate-js/view.js',
  '../../../vendor/foliate-js/overlayer.js',
])

export function foliate(): Promise<Foliate> {
  loaded ??= (async () => {
    const view = (await modules['../../../vendor/foliate-js/view.js']()) as { makeBook: Foliate['makeBook'] }
    const overlay = (await modules['../../../vendor/foliate-js/overlayer.js']()) as {
      Overlayer: { highlight: Draw; outline: Draw }
    }
    return {
      makeBook: view.makeBook,
      highlight: overlay.Overlayer.highlight,
      outline: overlay.Overlayer.outline,
    }
  })()
  return loaded
}

export class BookError extends Error {
  constructor(
    readonly reason: 'fixed-layout' | 'unreadable',
    message: string,
  ) {
    super(message)
  }
}

/** Reads the book, refuses a fixed-layout one, and puts a `<foliate-view>` for it into `host`. */
export async function openInto(host: HTMLElement, name: string, blob: Blob): Promise<FoliateView> {
  const lib = await foliate()
  let book: Book
  try {
    book = await lib.makeBook(new File([blob], name, { type: 'application/epub+zip' }))
  } catch (e) {
    throw new BookError('unreadable', (e as Error)?.message || 'This file could not be read as a book.')
  }
  if (book.rendition?.layout === 'pre-paginated')
    throw new BookError('fixed-layout', 'This book has a fixed layout, which the reader does not show.')
  const view = document.createElement('foliate-view') as FoliateView
  host.append(view)
  await view.open(book)
  return view
}

/** The title of a book as text (it may be given for several languages). */
export function titleOf(book: Book): string {
  const t = book.metadata?.title
  if (!t) return ''
  if (typeof t === 'string') return t
  return Object.values(t)[0] ?? ''
}
