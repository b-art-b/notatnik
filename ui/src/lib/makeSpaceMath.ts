// The arithmetic of "Make space" (DR-12), apart from js-draw so it can be tested alone. Canvas y grows downwards.

/** The items whose top edge is at or below the line: these move with it. A stroke that crosses the line stays. */
export function belowLine<T>(items: readonly T[], topOf: (item: T) => number, line: number): T[] {
  return items.filter((it) => topOf(it) >= line)
}

/** How far the lower part may move: down as far as wanted, up only until its highest item reaches the line, so
 * nothing is ever pushed above the line (and over the strokes there). */
export function clampShift(dy: number, tops: readonly number[], line: number): number {
  if (tops.length === 0) return 0
  return Math.max(dy, line - Math.min(...tops))
}
