// Renaming one segment of a page's path renames the namespace it belongs to.

export interface PrefixRename {
  /** The namespace as it is named now (`P/CapGemini`). */
  from: string
  /** The namespace as it will be named (`P/Capgemini12`). */
  to: string
}

/**
 * `P/CapGemini/sth` -> `P/Capgemini12/sth`: exactly one segment before the last one differs and the last one is
 * unchanged. Then the user most likely means "rename `CapGemini`", not "move this one page". Anything else
 * (several segments, the last segment too, a different depth) is an ordinary move of one page.
 */
export function prefixRename(old: string, next: string): PrefixRename | null {
  const a = old.split('/')
  const b = next.split('/').map((s) => s.trim())
  if (a.length < 2 || a.length !== b.length) return null
  const last = a.length - 1
  if (a[last] !== b[last]) return null
  const changed = a.flatMap((s, i) => (i < last && s !== b[i] ? [i] : []))
  if (changed.length !== 1 || !b[changed[0]]) return null
  const i = changed[0]
  return { from: a.slice(0, i + 1).join('/'), to: b.slice(0, i + 1).join('/') }
}

/** The page's name after its namespace was renamed. */
export function underPrefix(name: string, p: PrefixRename): string {
  return name.toLowerCase() === p.from.toLowerCase() ||
    name.toLowerCase().startsWith(p.from.toLowerCase() + '/')
    ? p.to + name.slice(p.from.length)
    : name
}
