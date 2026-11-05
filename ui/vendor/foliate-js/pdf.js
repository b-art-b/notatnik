// Not part of foliate-js. foliate-js has an experimental adapter for PDF.js here; this app opens PDFs in its own viewer
// (PdfViewer.svelte), so the adapter and the 10 MB of PDF.js it brings are left out. `makeBook` only reaches this file for
// a PDF, which the app never hands to the EPUB reader.
export const makePDF = async () => {
    throw new Error('PDFs are opened by the PDF viewer, not by foliate-js')
}
