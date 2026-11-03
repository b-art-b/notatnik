// The arithmetic of "Make space" (DR-12), apart from js-draw so it can be tested alone. Canvas y grows downwards, x to
// the right. The same functions serve both directions: a horizontal line (space made up or down, `y`) and a vertical
// line (space made left or right, `x`); `edgeOf` is the item's top edge for the first and its left edge for the second.

export type Axis = 'x' | 'y'

/** The items whose edge is at or past the line (below it, or to the right of it): these move with it. A stroke that
 * crosses the line stays. */
export function belowLine<T>(items: readonly T[], edgeOf: (item: T) => number, line: number): T[] {
  return items.filter((it) => edgeOf(it) >= line)
}

/** How far the far part may move: away from the line as far as wanted, towards it only until its nearest item reaches
 * the line, so nothing is ever pushed past the line (and over the strokes there). */
export function clampShift(delta: number, edges: readonly number[], line: number): number {
  if (edges.length === 0) return 0
  return Math.max(delta, line - Math.min(...edges))
}

/** Which way a drag goes, once it has moved `threshold` screen pixels: mostly sideways makes a vertical line (`x`),
 * otherwise a horizontal one (`y`). null while it is still too short to tell. */
export function pickAxis(dx: number, dy: number, threshold: number): Axis | null {
  if (Math.hypot(dx, dy) < threshold) return null
  return Math.abs(dx) > Math.abs(dy) ? 'x' : 'y'
}
