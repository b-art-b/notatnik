// A line diff for the page history: which lines of `a` were removed and which lines of `b` were added.

export type DiffLine = { kind: 'same' | 'del' | 'add'; text: string }

/** Line-by-line difference of two texts (longest common subsequence of lines, after trimming equal ends). */
export function diffLines(a: string, b: string): DiffLine[] {
  const x = a === '' ? [] : a.split('\n')
  const y = b === '' ? [] : b.split('\n')
  let start = 0
  while (start < x.length && start < y.length && x[start] === y[start]) start++
  let endX = x.length
  let endY = y.length
  while (endX > start && endY > start && x[endX - 1] === y[endY - 1]) {
    endX--
    endY--
  }
  const out: DiffLine[] = x.slice(0, start).map((text) => ({ kind: 'same', text }))
  const mx = x.slice(start, endX)
  const my = y.slice(start, endY)
  if (mx.length * my.length > 4_000_000) {
    // too big for the table: show the middle as one removal and one addition
    for (const text of mx) out.push({ kind: 'del', text })
    for (const text of my) out.push({ kind: 'add', text })
  } else {
    // lcs[i][j]: length of the common subsequence of mx[i..] and my[j..]
    const w = my.length + 1
    const lcs = new Uint32Array((mx.length + 1) * w)
    for (let i = mx.length - 1; i >= 0; i--) {
      for (let j = my.length - 1; j >= 0; j--) {
        lcs[i * w + j] =
          mx[i] === my[j] ? lcs[(i + 1) * w + j + 1] + 1 : Math.max(lcs[(i + 1) * w + j], lcs[i * w + j + 1])
      }
    }
    let i = 0
    let j = 0
    while (i < mx.length && j < my.length) {
      if (mx[i] === my[j]) {
        out.push({ kind: 'same', text: mx[i] })
        i++
        j++
      } else if (lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
        out.push({ kind: 'del', text: mx[i++] })
      } else {
        out.push({ kind: 'add', text: my[j++] })
      }
    }
    while (i < mx.length) out.push({ kind: 'del', text: mx[i++] })
    while (j < my.length) out.push({ kind: 'add', text: my[j++] })
  }
  for (const text of x.slice(endX)) out.push({ kind: 'same', text })
  return out
}

/** True when the two texts differ. */
export const changed = (d: DiffLine[]) => d.some((l) => l.kind !== 'same')
