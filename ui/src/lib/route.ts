// Hash-based routes: #/journals, #/page/<name>[?zoom=0.1.2]
export type Route =
  { kind: 'journals' } | { kind: 'page'; name: string; zoom: number[] } | { kind: 'settings' }

export function parseRoute(hash: string): Route {
  const h = hash.replace(/^#/, '')
  const [path, query = ''] = h.split('?')
  if (path.startsWith('/page/')) {
    const name = decodeURIComponent(path.slice('/page/'.length))
    const zoom = new URLSearchParams(query).get('zoom')
    return {
      kind: 'page',
      name,
      zoom: zoom
        ? zoom
            .split('.')
            .map(Number)
            .filter((n) => Number.isInteger(n))
        : [],
    }
  }
  if (path === '/settings') return { kind: 'settings' }
  return { kind: 'journals' }
}

export function routeHash(r: Route): string {
  switch (r.kind) {
    case 'journals':
      return '#/journals'
    case 'settings':
      return '#/settings'
    case 'page':
      return `#/page/${encodeURIComponent(r.name)}${r.zoom.length ? `?zoom=${r.zoom.join('.')}` : ''}`
  }
}

export const pageHash = (name: string, zoom: number[] = []) => routeHash({ kind: 'page', name, zoom })
