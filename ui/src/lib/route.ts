// Hash-based routes: #/journals, #/page/<name>[?zoom=0.1.2]
export type Route =
  | { kind: 'journals' }
  | { kind: 'page'; name: string; zoom: number[] }
  | { kind: 'settings' }
  | { kind: 'agenda' }
  | { kind: 'pages' }
  | { kind: 'property'; key: string; value: string | null }
  | { kind: 'blockprop'; key: string; value: string | null }
  | { kind: 'segment'; name: string }
  | { kind: 'saved'; name: string }

/** A route and the graph it is in: `#/g/1/page/X` is a page of the graph in slot 1, `#/page/X` one of the first graph
 * (slot `''`, which has no prefix, so links written before there were two graphs keep working). */
export interface Located {
  slot: string
  route: Route
}

export function parseHash(hash: string): Located {
  let h = hash.replace(/^#/, '')
  let slot = ''
  const g = /^\/g\/([a-z0-9]{1,8})(\/.*|\?.*)?$/.exec(h)
  if (g) {
    slot = g[1]
    h = g[2]?.startsWith('?') ? `/journals${g[2]}` : (g[2] ?? '/journals')
  }
  return { slot, route: parseRoute(h) }
}

/** The slot that links are written for (the active graph's): `pageHash(...)` and the other helpers add its prefix. */
let hashSlot = ''
export function setRouteSlot(slot: string) {
  hashSlot = slot
}

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
  if (path === '/agenda') return { kind: 'agenda' }
  if (path === '/pages') return { kind: 'pages' }
  if (path.startsWith('/segment/'))
    return { kind: 'segment', name: decodeURIComponent(path.slice('/segment/'.length)) }
  if (path.startsWith('/property/')) {
    const value = new URLSearchParams(query).get('value')
    return { kind: 'property', key: decodeURIComponent(path.slice('/property/'.length)), value }
  }
  if (path.startsWith('/block-property/')) {
    const value = new URLSearchParams(query).get('value')
    return { kind: 'blockprop', key: decodeURIComponent(path.slice('/block-property/'.length)), value }
  }
  if (path.startsWith('/saved/'))
    return { kind: 'saved', name: decodeURIComponent(path.slice('/saved/'.length)) }
  return { kind: 'journals' }
}

export function routeHash(r: Route, slot = hashSlot): string {
  const h = plainHash(r)
  return slot ? `#/g/${slot}${h.slice(1)}` : h
}

function plainHash(r: Route): string {
  switch (r.kind) {
    case 'journals':
      return '#/journals'
    case 'settings':
      return '#/settings'
    case 'agenda':
      return '#/agenda'
    case 'pages':
      return '#/pages'
    case 'segment':
      return `#/segment/${encodeURIComponent(r.name)}`
    case 'property':
      return `#/property/${encodeURIComponent(r.key)}${r.value === null ? '' : `?value=${encodeURIComponent(r.value)}`}`
    case 'blockprop':
      return `#/block-property/${encodeURIComponent(r.key)}${r.value === null ? '' : `?value=${encodeURIComponent(r.value)}`}`
    case 'saved':
      return `#/saved/${encodeURIComponent(r.name)}`
    case 'page':
      return `#/page/${encodeURIComponent(r.name)}${r.zoom.length ? `?zoom=${r.zoom.join('.')}` : ''}`
  }
}

export const pageHash = (name: string, zoom: number[] = []) => routeHash({ kind: 'page', name, zoom })

/** A link to a page of the graph in `slot`, whichever graph is active (a sidebar section's links). */
export const pageHashIn = (slot: string, name: string, zoom: number[] = []) =>
  routeHash({ kind: 'page', name, zoom }, slot)

/** The view of all pages that have a page property (optionally with one value). */
export const propertyHash = (key: string, value: string | null = null) =>
  routeHash({ kind: 'property', key, value })

/** The view of all blocks that have a block property (optionally with one value). */
export const blockPropertyHash = (key: string, value: string | null = null) =>
  routeHash({ kind: 'blockprop', key, value })

/** The view of every place where a name occurs as a path segment (`Renewal` in `P/EY/Renewal/APotheke`). */
export const segmentHash = (name: string) => routeHash({ kind: 'segment', name })
