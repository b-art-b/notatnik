// Zoom of the drawing canvas (DR-19): the steps and the label, apart from js-draw so they can be tested alone.

/** The scale after one step in or out: a quarter bigger or smaller, within 2 % and 3200 %. */
export function zoomed(scale: number, dir: 1 | -1): number {
  const next = dir > 0 ? scale * 1.25 : scale / 1.25
  return Math.min(32, Math.max(0.02, next))
}

/** The label of a scale: whole percent. */
export function percent(scale: number): string {
  return `${Math.round(scale * 100)} %`
}
