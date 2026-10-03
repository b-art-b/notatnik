import type { GraphChangedEvent } from './bindings'
import { type Backend, CommandError } from './types'

function apiBase(): string {
  const fromQuery = new URLSearchParams(window.location.search).get('api')
  if (fromQuery) {
    try {
      sessionStorage.setItem('notatnik-api', fromQuery)
    } catch {
      /* sessionStorage can be unavailable; the query parameter is enough for this load */
    }
    return fromQuery
  }
  try {
    return sessionStorage.getItem('notatnik-api') ?? 'http://127.0.0.1:8787'
  } catch {
    return 'http://127.0.0.1:8787'
  }
}

/** Talks to `notatnik-dev-server` over HTTP; events arrive by long-polling. */
export function createHttpBackend(): Backend {
  const base = apiBase()
  return {
    async call(name, args) {
      const res = await fetch(`${base}/api/${name}`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(args),
      })
      const body = await res.json()
      if (!res.ok) throw new CommandError(body)
      return body
    },
    onGraphChanged(cb) {
      let stopped = false
      let after = 0
      const loop = async () => {
        while (!stopped) {
          try {
            const res = await fetch(`${base}/poll?after=${after}`)
            const body = (await res.json()) as {
              last: number
              events: { id: number; name: string; payload: GraphChangedEvent }[]
            }
            after = body.last
            for (const e of body.events) if (e.name === 'graph-changed') cb(e.payload)
          } catch {
            await new Promise((r) => setTimeout(r, 500))
          }
        }
      }
      void loop()
      return () => {
        stopped = true
      }
    },
    assetUrl: (path) => `${base}/asset?path=${encodeURIComponent(path.replace(/^(\.\.\/)+/, ''))}`,
    openExternal: (url) => {
      window.open(url, '_blank', 'noopener,noreferrer')
    },
  }
}
