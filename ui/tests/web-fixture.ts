import http from 'node:http'
import type { AddressInfo } from 'node:net'

// A small web server for the clip tests (WA-1..WA-10): pages, a picture, a redirect, a slow page, an error page.

/** The sentence the tests highlight; it is in the page once. */
export const FIRST =
  'The harbour town woke slowly, as it always did in the grey weeks before the first ferry of the year.'
/** The sentence after it. */
export const SECOND =
  'Nobody on the quay could say who had left the lantern burning in the old customs house overnight.'

/** Eight paragraphs; the first holds the two sentences above, the others say something of their own, so that no
 * sentence is in the page twice (a highlight must find its own words). */
function paragraphs(n = 8): string {
  return Array.from({ length: n }, (_, i) =>
    i === 0
      ? `<p>${FIRST} ${SECOND}</p>`
      : `<p>On day ${i} the keeper wrote line number ${i} of the log and found nothing in it worth a second sentence.</p>`,
  ).join('\n')
}

/** The article, with a picture, a navigation bar and a script around it. `extra` is added above the first paragraph. */
export function article(title = 'The Lantern', extra = ''): string {
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>${title} | Harbour News</title></head>
<body>
<nav><a href="/">Home</a> <a href="/about">About</a></nav>
<article>
<h1>${title}</h1>
<p class="byline">By Ada Quay</p>
${extra}
${paragraphs()}
<p><img src="/pic.png" alt="A lantern"></p>
</article>
<footer>Subscribe to our newsletter</footer>
<script>document.title = 'changed by the page'</script>
</body></html>`
}

// a 1x1 PNG
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
)

export interface PageServer {
  /** `http://127.0.0.1:<port>` */
  base: string
  /** What `/article` serves now; set it to change the page between two clips. */
  articleHtml: string
  /** How many times `/slow` was asked for. */
  slowHits: () => number
  close(): Promise<void>
}

export async function pageServer(): Promise<PageServer> {
  let slow = 0
  const s: PageServer = {
    base: '',
    articleHtml: article(),
    slowHits: () => slow,
    close: () =>
      new Promise((r) => {
        server.close(() => r())
        // the slow page still holds its connection
        server.closeAllConnections()
      }),
  }
  const server = http.createServer((req, res) => {
    const url = (req.url ?? '/').split('?')[0]
    if (url === '/article') {
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }).end(s.articleHtml)
    } else if (url === '/hostile') {
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }).end(hostilePage())
    } else if (url === '/pic.png') {
      res.writeHead(200, { 'content-type': 'image/png' }).end(PNG)
    } else if (url === '/moved') {
      res.writeHead(302, { location: '/article' }).end()
    } else if (url === '/slow') {
      slow++
      // never answers within a test: the dialog has to be cancelled
      const late = setTimeout(
        () => res.writeHead(200, { 'content-type': 'text/html' }).end(article('Slow')),
        20_000,
      )
      res.on('close', () => clearTimeout(late))
    } else if (url === '/data.json') {
      res.writeHead(200, { 'content-type': 'application/json' }).end('{"a":1}')
    } else {
      res.writeHead(404, { 'content-type': 'text/plain' }).end('not here')
    }
  })
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r))
  s.base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`
  return s
}

/** A page that tries everything: scripts, event handlers, a remote picture, a remote style sheet, a frame, a form. The
 * article text is the only thing that should come out of a clip. */
export function hostilePage(): string {
  const filler = Array.from(
    { length: 8 },
    (_, i) =>
      `<p>Paragraph ${i} of the hostile article, long enough to be taken for the article itself by the reader.</p>`,
  ).join('\n')
  return `<!doctype html><html><head><meta charset="utf-8"><title>Hostile | Site</title>
<link rel="stylesheet" href="https://leak.invalid/site.css"><style>@import url("https://leak.invalid/a.css");</style>
<script>window.__pwned = 'head'</script></head>
<body><article><h1>Hostile</h1>
<script>fetch('https://leak.invalid/s'); window.parent.__pwned = 'script'</script>
<img src="https://leak.invalid/x.png" alt="remote" onerror="window.parent.__pwned = 'onerror'">
<iframe src="https://leak.invalid/frame"></iframe>
<form action="https://leak.invalid/post"><input name="a"></form>
<p id="p1" onclick="window.parent.__pwned = 'onclick'">Safe text before the traps. <a id="js" href="javascript:window.parent.__pwned = 'javascript'">a javascript link</a></p>
${filler}
</article></body></html>`
}
