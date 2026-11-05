// pdf.js, loaded the first time a PDF is opened (PA-1, PA-10): the library and its worker are separate files that the
// first page of the app never fetches. The `legacy` build runs on the older WebKit of some Linux systems and iPads.
import type { PDFDocumentProxy } from 'pdfjs-dist'

type PdfJs = typeof import('pdfjs-dist')

let lib: Promise<PdfJs> | null = null

export function pdfjs(): Promise<PdfJs> {
  lib ??= (async () => {
    const [mod, worker] = await Promise.all([
      import('pdfjs-dist/legacy/build/pdf.mjs') as Promise<PdfJs>,
      import('pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'),
    ])
    mod.GlobalWorkerOptions.workerSrc = worker.default
    return mod
  })()
  return lib
}

/** Why a PDF could not be shown, for the message. */
export type PdfFailure = 'password' | 'wrong-password' | 'invalid' | 'missing' | 'other'

export class PdfError extends Error {
  constructor(
    readonly failure: PdfFailure,
    message: string,
  ) {
    super(message)
  }
}

/** Opens the PDF at `url`. A PDF with a password calls `askPassword` (once per attempt); null cancels. */
export async function openDocument(
  url: string,
  askPassword: (wrong: boolean) => Promise<string | null>,
): Promise<PDFDocumentProxy> {
  const lib = await pdfjs()
  const res = await fetch(url).catch(() => null)
  if (!res || !res.ok) throw new PdfError('missing', 'The file could not be read.')
  const data = new Uint8Array(await res.arrayBuffer())
  const task = lib.getDocument({
    data,
    cMapUrl: `${import.meta.env.BASE_URL}pdfjs/cmaps/`,
    cMapPacked: true,
    standardFontDataUrl: `${import.meta.env.BASE_URL}pdfjs/standard_fonts/`,
    isEvalSupported: false,
  })
  task.onPassword = (reply: (pw: string) => void, reason: number) => {
    void askPassword(reason === lib.PasswordResponses.INCORRECT_PASSWORD).then((pw) => {
      if (pw === null) void task.destroy()
      else reply(pw)
    })
  }
  try {
    return await task.promise
  } catch (e) {
    const name = (e as { name?: string })?.name
    if (name === 'PasswordException') throw new PdfError('password', 'This PDF needs a password.')
    if (name === 'InvalidPDFException') throw new PdfError('invalid', 'This file is not a PDF.')
    if (name === 'MissingPDFException') throw new PdfError('missing', 'The file could not be read.')
    throw new PdfError('other', (e as Error)?.message ?? 'The PDF could not be opened.')
  }
}
