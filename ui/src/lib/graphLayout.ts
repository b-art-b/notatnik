// The picture of the local graph (LN-9): a fixed layout, no physics, so the same page always looks the same.
// The page is in the middle, the pages one step away on a ring, those two steps away on an outer ring, each beside
// the page that brought it in.

export interface LayoutNode {
  name: string
  depth: number
}
export interface LayoutEdge {
  from: string
  to: string
}
export interface Placed {
  name: string
  depth: number
  x: number
  y: number
  /** The angle from the middle, in radians: where the label goes (outwards). */
  angle: number
}
export interface Layout {
  width: number
  height: number
  nodes: Placed[]
}

/** Room one node needs along its ring, in picture units. */
const SLOT = 26
const MARGIN = 70 // room for the labels beyond the last ring

export function layoutGraph(nodes: LayoutNode[], edges: LayoutEdge[]): Layout {
  const byName = (a: string, b: string) => a.localeCompare(b)
  const center = nodes.find((n) => n.depth === 0)
  const ring1 = nodes.filter((n) => n.depth === 1).sort((a, b) => byName(a.name, b.name))
  const ring2 = nodes.filter((n) => n.depth === 2).sort((a, b) => byName(a.name, b.name))
  // the rings are as wide as their nodes need: a full ring of 59 pages is a big picture, which scales to the panel
  const r1 = Math.max(95, (ring1.length * SLOT) / (2 * Math.PI))
  const r2 = Math.max(r1 + 75, (ring2.length * SLOT) / (2 * Math.PI))
  const outer = ring2.length ? r2 : r1
  const half = outer + MARGIN
  const mid = half
  const placed: Placed[] = []
  const at = (n: LayoutNode, r: number, angle: number) =>
    placed.push({
      name: n.name,
      depth: n.depth,
      x: Math.round((mid + r * Math.cos(angle)) * 10) / 10,
      y: Math.round((mid + r * Math.sin(angle)) * 10) / 10,
      angle,
    })
  if (center) at(center, 0, 0)

  const angle1 = new Map<string, number>()
  ring1.forEach((n, i) => {
    const a = -Math.PI / 2 + (i * 2 * Math.PI) / ring1.length
    angle1.set(n.name, a)
    at(n, r1, a)
  })

  // two steps away: beside the first (by name) page of ring 1 that links with it, in either direction
  const parent = new Map<string, string>()
  for (const n of ring2) {
    const p = ring1
      .filter((r) =>
        edges.some((e) => (e.from === r.name && e.to === n.name) || (e.from === n.name && e.to === r.name)),
      )
      .map((r) => r.name)[0]
    if (p) parent.set(n.name, p)
  }
  const groups = new Map<string, LayoutNode[]>()
  const loose: LayoutNode[] = []
  for (const n of ring2) {
    const p = parent.get(n.name)
    if (p) groups.set(p, [...(groups.get(p) ?? []), n])
    else loose.push(n)
  }
  // the pages of one group are spread over the slot their parent has, wide enough for each to keep its own place
  const slot = ring2.length ? (2 * Math.PI) / ring2.length : 0
  const used: { a: number; n: LayoutNode }[] = []
  for (const [p, list] of groups) {
    const base = angle1.get(p) ?? 0
    list.forEach((n, i) => used.push({ a: base + (i - (list.length - 1) / 2) * slot, n }))
  }
  // groups can crowd each other: all of ring 2 is put in angle order and then spaced at least one slot apart
  used.sort((x, y) => x.a - y.a || byName(x.n.name, y.n.name))
  const all = [...used.map((u) => u.n), ...loose]
  const baseAngles = [...used.map((u) => u.a), ...loose.map((_, i) => Math.PI / 2 + i * slot)]
  const order = all.map((n, i) => ({ n, a: baseAngles[i] }))
  order.sort((x, y) => x.a - y.a || byName(x.n.name, y.n.name))
  const spaced: number[] = []
  order.forEach((o, i) => {
    const min = i === 0 ? o.a : spaced[i - 1] + slot
    spaced.push(Math.max(o.a, min))
  })
  // if that pushed the last past a full turn, the whole ring is shared out evenly instead
  const turn = spaced.length ? spaced[spaced.length - 1] - spaced[0] + slot : 0
  order.forEach((o, i) => {
    const a = turn > 2 * Math.PI + 1e-9 ? -Math.PI / 2 + i * slot : spaced[i]
    at(o.n, r2, a)
  })
  return { width: 2 * half, height: 2 * half, nodes: placed }
}

/** A name for the picture: long names are cut with an ellipsis (the whole name is the title and the text list). */
export function shortName(name: string, max = 16): string {
  if (name.length <= max) return name
  const leaf = name.includes('/') ? name.slice(name.lastIndexOf('/') + 1) : name
  const base = leaf.length < name.length && leaf.length >= 4 ? `…/${leaf}` : name
  return base.length <= max ? base : `${base.slice(0, max - 1)}…`
}
