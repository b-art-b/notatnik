// Which block properties are shown as a property card (PR-6): not hidden ones, and for a task not the ones that
// have their own chips and indicators.
import type { BlockView, PropView } from './api'

const CHIP_KEYS = ['scheduled', 'deadline', 'priority', 'repeat']

export function shownProps(block: BlockView): PropView[] {
  return block.props.filter((p) => !p.hidden && !(block.task && CHIP_KEYS.includes(p.key)))
}

/** `[[A]], [[B]]` → `['[[A]]', '[[B]]']`; null when the text is anything else. */
export function linkList(text: string): string[] | null {
  const parts = text.split(',').map((p) => p.trim())
  return parts.length > 0 && parts.every((p) => /^\[\[[^[\]]+\]\]$/.test(p)) ? parts : null
}

/**
 * A page property value as the frontmatter writes it. Page links are quoted (`"[[Page]]"`, several as a YAML list
 * of quoted links): unquoted `[[Page]]` is YAML for a list in a list. A plain word stays as it is; anything YAML
 * could misread (`a: b`, `#x`, ...) is quoted.
 */
export function yamlValue(v: string): string {
  const t = v.trim()
  const links = linkList(t)
  if (links)
    return links.length === 1
      ? JSON.stringify(links[0])
      : `[${links.map((l) => JSON.stringify(l)).join(', ')}]`
  if (/^\[.*\]$/.test(t) || /^[\p{L}\p{N}][\p{L}\p{N} ._/@+-]*$/u.test(t)) return t
  return JSON.stringify(t)
}

/** What the value field shows for a raw frontmatter value: quotes removed, a list of links as `[[A]], [[B]]`. */
export function displayValue(raw: string): string {
  const t = raw.trim()
  if (/^"(?:[^"\\]|\\.)*"$/.test(t)) {
    try {
      return String(JSON.parse(t))
    } catch {
      return t
    }
  }
  if (/^'[^']*'$/.test(t)) return t.slice(1, -1)
  if (/^\[\s*"\[\[/.test(t)) {
    try {
      const a: unknown = JSON.parse(t)
      if (Array.isArray(a) && a.every((x) => typeof x === 'string' && linkList(x))) return a.join(', ')
    } catch {
      /* not JSON: shown as written */
    }
  }
  return t
}

/** The page names in a value written with `[[ ]]`. */
export function linkedPages(value: string): string[] {
  return [...value.matchAll(/\[\[([^[\]|]+)(?:\|[^\]]*)?\]\]/g)].map((m) => m[1].trim())
}
