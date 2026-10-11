// Autocomplete in the block editor: [[page, ((block, #tag and /commands (ED-7, NS-7).
import {
  autocompletion,
  startCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import type { Extension } from '@codemirror/state'
import { backend, type TitleHit } from './api'
import {
  logChanges,
  logStamp,
  markerChanges,
  propertyChanges,
  propertyEnd,
  rankCommands,
  SLASH_GROUPS,
  slashAt,
  slashPool,
  type DateKind,
} from './commands/slash'
import { EditorView } from '@codemirror/view'
import { searchEmoji } from './emoji'

/** A slash-command completion; `icon` and `description` are drawn by `slashPart`. */
type SlashCompletion = Completion & { icon?: string; description?: string }

/** The extra pieces of a slash-command row (other completions get none). */
function slashPart(c: Completion, part: 'icon' | 'desc' | 'hint'): HTMLElement | null {
  if (c.type !== 'slash') return null
  const s = c as SlashCompletion
  const el = document.createElement('span')
  el.className = `slash-${part}`
  el.textContent = part === 'icon' ? (s.icon ?? '') : part === 'desc' ? (s.description ?? '') : c.label
  if (part === 'icon') el.setAttribute('aria-hidden', 'true')
  return el
}

/** Look of every completion popup: card, comfortable rows, a clear selected row and a key-hint footer.
 * Colours come from the app's CSS variables, so light and dark themes both work. */
const menuTheme = EditorView.theme({
  '.cm-tooltip.cm-tooltip-autocomplete': {
    background: 'var(--panel)',
    color: 'var(--text)',
    border: '1px solid var(--border)',
    borderRadius: '10px',
    boxShadow: '0 8px 28px rgb(0 0 0 / 0.18), 0 1px 3px rgb(0 0 0 / 0.1)',
    padding: '4px',
    minWidth: '300px',
    maxWidth: 'min(460px, 90vw)',
    fontFamily: 'inherit',
    overflow: 'hidden',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': {
    fontFamily: 'inherit',
    maxHeight: '25em',
    padding: '0',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > completion-section': {
    display: 'block',
    padding: '8px 8px 3px',
    fontSize: '0.72em',
    fontWeight: '600',
    letterSpacing: '0.06em',
    textTransform: 'uppercase',
    color: 'var(--muted)',
    borderBottom: 'none',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
    padding: '5px 8px',
    borderRadius: '7px',
    lineHeight: '1.3',
    display: 'flex',
    alignItems: 'baseline',
    gap: '0.5em',
  },
  '.cm-tooltip-autocomplete ul li[aria-selected]': {
    background: 'var(--selection)',
    color: 'var(--text)',
  },
  '.cm-tooltip-autocomplete .cm-completionLabel': { fontSize: '0.95em' },
  '.cm-tooltip-autocomplete .cm-completionDetail': {
    color: 'var(--muted)',
    fontStyle: 'normal',
    marginLeft: 'auto',
    fontSize: '0.8em',
  },
  '.cm-tooltip-autocomplete .cm-completionMatchedText': {
    textDecoration: 'none',
    fontWeight: '700',
    color: 'var(--accent)',
  },
  // slash rows: [icon tile] [label / description] [/name]
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li.slash-option': {
    display: 'grid',
    gridTemplateColumns: '30px 1fr auto',
    gridTemplateAreas: '"icon label hint" "icon desc hint"',
    columnGap: '10px',
    alignItems: 'center',
    padding: '5px 8px',
  },
  '.slash-option .slash-icon': {
    gridArea: 'icon',
    width: '30px',
    height: '30px',
    borderRadius: '7px',
    background: 'var(--chip)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: '0.95em',
    color: 'var(--text)',
  },
  '.cm-tooltip-autocomplete li.slash-option[aria-selected] .slash-icon': {
    background: 'var(--panel)',
  },
  '.slash-option .cm-completionLabel': { gridArea: 'label', fontWeight: '600' },
  '.slash-option .slash-desc': {
    gridArea: 'desc',
    fontSize: '0.8em',
    color: 'var(--muted)',
    whiteSpace: 'nowrap',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
  },
  '.slash-option .slash-hint': {
    gridArea: 'hint',
    fontFamily: 'var(--font-mono)',
    fontSize: '0.78em',
    color: 'var(--muted)',
    background: 'var(--chip)',
    borderRadius: '5px',
    padding: '1px 6px',
  },
  // keyboard hints under the list
  '.cm-tooltip.cm-tooltip-autocomplete::after': {
    content: '"↑↓ navigate   ↵ select   esc dismiss"',
    whiteSpace: 'pre',
    display: 'block',
    padding: '6px 8px 3px',
    marginTop: '3px',
    borderTop: '1px solid var(--border)',
    fontSize: '0.72em',
    color: 'var(--muted)',
  },
})

export const QUERY_KEYS = [
  'task',
  'tag',
  'links',
  'ns',
  'page',
  'path',
  'scheduled',
  'deadline',
  'text',
  'type',
  'sort',
  'limit',
  'group',
  'view',
  'columns',
]

/** Where the cursor is: inside a ```query fence, and on which line of the block text. */
export function cursorContext(
  doc: string,
  pos: number,
): { inQuery: boolean; inFence: boolean; line: number; before: string } {
  const lines = doc.slice(0, pos).split('\n')
  let fence: string | null = null
  for (let i = 0; i < lines.length - 1; i++) {
    const m = /^\s*(?:```|~~~)(.*)$/.exec(lines[i])
    if (m) fence = fence === null ? m[1].trim() : null
  }
  // a fence opened on the first line (`- ```query`) is part of that line
  return {
    inQuery: fence !== null && fence.startsWith('query'),
    inFence: fence !== null,
    line: lines.length - 1,
    before: lines[lines.length - 1],
  }
}

/** Ranks pages of the current namespace (and its siblings) first, keeping fuzzy order otherwise. */
export function rankTitles(hits: TitleHit[], page: string, typed: string): TitleHit[] {
  const ns = page.includes('/') ? page.slice(0, page.lastIndexOf('/')) : ''
  const score = (h: TitleHit): number => {
    if (typed.includes('/')) return 0
    if (!ns) return 0
    if (h.name.startsWith(ns + '/')) return -2
    if (ns.includes('/') && h.name.startsWith(ns.slice(0, ns.lastIndexOf('/')) + '/')) return -1
    return 0
  }
  return hits
    .map((h, i) => ({ h, i }))
    .sort((a, b) => score(a.h) - score(b.h) || a.i - b.i)
    .map((x) => x.h)
}

/** What a property line asks for, from the text before the cursor on a continuation line:
 * `value` (`key:: va`), `bare` (a key being typed, known keys only), `colons` (`ca::` or an empty `::`: pick or
 * create a key) or `picked` (`::ca`: the colons came first, then the filter). */
export type PropertyTrigger =
  { kind: 'value'; key: string; typed: string } | { kind: 'bare' | 'colons' | 'picked'; typed: string }

/**
 * In a query block: the cursor is in the value of `prop.<key>:`. `typed` is the text of the value being written
 * (after an operator such as `=` or `contains`, after an earlier alternative and its comma); `bare` is that text
 * without the brackets of a page link.
 */
export function queryPropValueTrigger(before: string): { key: string; typed: string; bare: string } | null {
  const m =
    /^\s*-?prop\.([A-Za-z0-9_.-]+):\s*(?:.*,\s*)?(?:(?:!=|<=|>=|<|>|=|contains\s)\s*)?([^,\n]*)$/.exec(before)
  if (!m) return null
  const typed = m[2]
  return { key: m[1], typed, bare: typed.replace(/^\[\[/, '').replace(/\]\]$/, '') }
}

export function propertyTrigger(before: string): PropertyTrigger | null {
  let m = /^\s*([a-z0-9_-]+)::\s(.*)$/.exec(before)
  if (m) return { kind: 'value', key: m[1], typed: m[2] }
  m = /^\s*([a-z0-9_-]*)::$/.exec(before)
  if (m) return { kind: 'colons', typed: m[1] }
  m = /^\s*::([a-z0-9_-]*)$/.exec(before)
  if (m) return { kind: 'picked', typed: m[1] }
  m = /^\s*([a-z0-9_-]+)$/.exec(before)
  if (m) return { kind: 'bare', typed: m[1] }
  return null
}

/** Writes `key:: ` over the typed text and goes straight on to the value menu. */
function acceptKey(view: EditorView, key: string, from: number, to: number) {
  view.dispatch({
    changes: { from, to, insert: `${key}:: ` },
    selection: { anchor: from + key.length + 3 },
  })
  startCompletion(view)
}

async function titleCompletions(typed: string, page: string): Promise<Completion[]> {
  const query = typed.trim()
  let hits: TitleHit[] = []
  if (query.endsWith('/')) {
    // `work/` lists only the children of `work`
    const parent = query.replace(/\/+$/, '')
    const rows = await backend.call('list_namespace', { parent })
    hits = rows.map((r) => ({ name: r.name, is_virtual: r.is_virtual }))
  } else if (query) {
    hits = await backend.call('search_titles', { text: query, limit: 12 })
  } else {
    hits = (await backend.call('list_namespace', { parent: null })).map((r) => ({
      name: r.name,
      is_virtual: r.is_virtual,
    }))
  }
  hits = rankTitles(hits, page, typed)
  const out: Completion[] = hits.map((h) => ({
    label: h.name,
    type: h.is_virtual ? 'text' : 'variable',
    detail: h.is_virtual ? 'no file yet' : undefined,
  }))
  if (query && !hits.some((h) => h.name.toLowerCase() === query.toLowerCase())) {
    out.push({ label: query, displayLabel: `Create page “${query}”`, type: 'keyword', boost: -99 })
  }
  return out
}

/** `onpickdate(kind, at)` opens the date picker; `at` is where the removed `/command` started. */
export function wikiCompletions(
  pageName: () => string,
  onpickdate: (kind: DateKind, at: number) => void,
  onemoji: (at: number) => void,
  ontemplate: (name: string) => void = () => {},
): Extension {
  return [
    autocompletion({
      activateOnTyping: true,
      closeOnBlur: false,
      icons: false,
      optionClass: (c) => (c.type === 'slash' ? 'slash-option' : ''),
      maxRenderedOptions: 60,
      // slash commands get an icon tile before the label and a description + `/name` hint after it
      addToOptions: [
        { position: 20, render: (c) => slashPart(c, 'icon') },
        { position: 60, render: (c) => slashPart(c, 'desc') },
        { position: 70, render: (c) => slashPart(c, 'hint') },
      ],
      override: [
        async (ctx: CompletionContext): Promise<CompletionResult | null> => {
          const before = ctx.state.sliceDoc(Math.max(0, ctx.pos - 200), ctx.pos)
          // [[page
          let m = /\[\[([^\]\n|]*)$/.exec(before)
          if (m) {
            const from = ctx.pos - m[1].length
            const options = await titleCompletions(m[1], pageName())
            const after = ctx.state.sliceDoc(ctx.pos, ctx.pos + 2)
            return {
              from,
              options: options.map((o) => ({
                ...o,
                apply: (view, _c, f, t) =>
                  view.dispatch({
                    changes: { from: f, to: t, insert: o.label + (after === ']]' ? '' : ']]') },
                    selection: { anchor: f + o.label.length + (after === ']]' ? 2 : 2) },
                  }),
              })),
              filter: false,
            }
          }
          // ((text → block search; inserting calls ensure_block_id on the target block
          m = /\(\(([^)\n]*)$/.exec(before)
          if (m) {
            const q = m[1]
            if (q.length < 2) return { from: ctx.pos - q.length, options: [], filter: false }
            // the word being typed is a prefix: `unique tar` finds `unique target`
            const text = q.trim().replace(/(\S+)$/, (w) => (w.endsWith('*') ? w : `${w}*`))
            const hits = (await backend.call('search', { text, limit: 10 })).filter(
              (h) => h.kind === 'block' && h.path,
            )
            return {
              from: ctx.pos - q.length,
              filter: false,
              options: hits.map((h) => ({
                label: h.text.split('\n')[0].slice(0, 80),
                detail: h.page,
                type: 'text',
                apply: async (view, _c, f, t) => {
                  try {
                    const id = await backend.call('ensure_block_id', { page: h.page, path: h.path ?? [] })
                    const after = view.state.sliceDoc(t, t + 2)
                    view.dispatch({
                      changes: { from: f, to: t, insert: id.value + (after === '))' ? '' : '))') },
                    })
                  } catch {
                    /* the toast layer isn't reachable from here; the block ref simply isn't inserted */
                  }
                },
              })),
            }
          }
          // #tag
          m = /(?:^|[\s(])#([\p{L}\p{N}_/-]*)$/u.exec(before)
          if (m) {
            const typed = m[1]
            const options = await titleCompletions(typed, pageName())
            return {
              from: ctx.pos - typed.length,
              filter: false,
              options: options
                .filter((o) => !o.displayLabel)
                .map((o) => ({
                  ...o,
                  apply: (view, _c, f, t) => {
                    const simple = /^[\p{L}\p{N}_/-]+$/u.test(o.label) && !/[-/]$/.test(o.label)
                    if (simple) view.dispatch({ changes: { from: f, to: t, insert: o.label } })
                    else view.dispatch({ changes: { from: f - 1, to: t, insert: `#[[${o.label}]]` } })
                  },
                })),
            }
          }
          const cx = cursorContext(ctx.state.doc.toString(), ctx.pos)
          // clause keys inside a query block
          if (cx.inQuery) {
            const k = /^\s*-?([a-z.]*)$/.exec(cx.before)
            if (k) {
              const keys = await backend.call('property_keys', {})
              const all = [...QUERY_KEYS, ...keys.items.map((p) => `prop.${p}`)]
              return {
                from: ctx.pos - k[1].length,
                filter: true,
                options: all.map((key) => ({
                  label: key,
                  detail: 'clause',
                  type: 'keyword',
                  apply: `${key}: `,
                })),
              }
            }
            const pv = queryPropValueTrigger(cx.before)
            if (pv) {
              // `prop.partner: ` offers the values in use and pages, a page is written as `[[Page]]`
              const [vals, pages] = await Promise.all([
                backend.call('property_values', { key: pv.key }),
                titleCompletions(pv.bare, pageName()),
              ])
              const q = pv.bare.toLowerCase()
              return {
                from: ctx.pos - pv.typed.length,
                filter: false,
                options: [
                  ...vals.items
                    .filter((x) => !q || x.toLowerCase().includes(q))
                    .map((x): Completion => ({ label: x, type: 'text', detail: 'value' })),
                  ...pages
                    .filter((o) => !o.displayLabel)
                    .map((o): Completion => ({
                      label: o.label,
                      type: 'variable',
                      detail: 'page',
                      apply: `[[${o.label}]]`,
                    })),
                ],
              }
            }
            const v = /^\s*-?(?:tag|ns|page|links):\s*(?:.*,\s*)?([^,\n]*)$/.exec(cx.before)
            if (v) {
              const options = await titleCompletions(v[1], pageName())
              return {
                from: ctx.pos - v[1].length,
                filter: false,
                options: options.filter((o) => !o.displayLabel),
              }
            }
            return null
          }
          // a `/word` that starts a slash command wins over property completion (`deadline:: 2026-10-10 /log`)
          const slash = slashAt(before)
          const slashHit =
            !!slash &&
            !(slash.midWord && cx.inFence) &&
            rankCommands(slashPool(slash), slash.typed).length > 0
          // `:smile` → 😄 (a colon that starts a word; `key::`, `12:30` and `http://` are not shortcodes)
          const short = /(?:^|\s):([a-z0-9_+-]{2,})$/.exec(before)
          const shortHits = short && !cx.inFence ? searchEmoji(short[1], 8) : []
          // property lines (after the first line of a block): `key:: value`
          if (!cx.inFence && cx.line > 0 && !slashHit && shortHits.length === 0) {
            const trig = propertyTrigger(cx.before)
            if (trig?.kind === 'value') {
              const values = await backend.call('property_values', { key: trig.key })
              // values already used for this key, then pages (a page value is written as `[[Page]]`)
              const pages = trig.typed.startsWith('[[') ? [] : await titleCompletions(trig.typed, pageName())
              return {
                from: ctx.pos - trig.typed.length,
                filter: true,
                options: [
                  ...values.items.map((x): Completion => ({ label: x, type: 'text', detail: 'value' })),
                  ...pages
                    .filter((o) => !o.displayLabel)
                    .map((o): Completion => ({
                      label: o.label,
                      type: 'variable',
                      detail: 'page',
                      apply: `[[${o.label}]]`,
                    })),
                ],
              }
            }
            if (trig) {
              const keys = (await backend.call('property_keys', {})).items
              const typed = trig.typed
              const options: Completion[] = keys
                .filter((k) => k.includes(typed))
                .map((k) => ({
                  label: k,
                  detail: 'property',
                  type: 'property',
                  apply: (view, _c, f, to) => acceptKey(view, k, f, to),
                }))
              // a key nobody uses yet: `ca::` offers to create it (Logseq's "Create a new property")
              if (typed && trig.kind !== 'bare' && !keys.includes(typed)) {
                options.push({
                  label: typed,
                  displayLabel: `Create property: ${typed}`,
                  type: 'keyword',
                  boost: -99,
                  apply: (view, _c, f, to) => acceptKey(view, typed, f, to),
                })
              }
              if (options.length === 0) return null
              // `ca::` and `::ca` are replaced whole, so they are filtered here, not by CodeMirror
              const colons = trig.kind !== 'bare'
              return {
                from: ctx.pos - typed.length - (colons ? 2 : 0),
                filter: !colons,
                options,
              }
            }
          }
          if (short && shortHits.length > 0) {
            return {
              from: ctx.pos - short[1].length - 1,
              filter: false,
              options: shortHits.map((e): Completion => ({
                label: `:${e.name}:`,
                displayLabel: `${e.char}  ${e.name}`,
                type: 'emoji',
                apply: (view, _c, f, t) => {
                  view.dispatch({
                    changes: { from: f, to: t, insert: e.char },
                    selection: { anchor: f + e.char.length },
                  })
                },
              })),
            }
          }
          // /command: at the start of a word, or in the middle of one (`wor/d`, metadata commands only)
          const at = slashAt(before)
          if (at && !(at.midWord && cx.inFence)) {
            const typed = at.typed
            const pool = slashPool(at)
            return {
              from: ctx.pos - typed.length - 1,
              filter: false, // ranked by `rankCommands`
              // grouped while nothing is typed, one relevance-ranked list once you type (no headers to scan)
              options: rankCommands(pool, typed).map((c): SlashCompletion => ({
                label: '/' + c.name,
                displayLabel: c.label,
                type: 'slash',
                section: typed ? undefined : { name: c.group, rank: SLASH_GROUPS.indexOf(c.group) },
                // read by `slashOptionParts` below
                icon: c.icon,
                description: c.title,
                apply: (view, _comp, f, t) => {
                  if (c.marker) {
                    view.dispatch({ changes: markerChanges(view.state.doc.toString(), f, t, c.marker) })
                    return
                  }
                  if (c.template) {
                    // the command goes away (with the space before it); the blocks are inserted by the host
                    const cut = view.state.sliceDoc(f - 1, f) === ' ' ? f - 1 : f
                    view.dispatch({ changes: { from: cut, to: t, insert: '' } })
                    ontemplate(c.template)
                    return
                  }
                  if (c.emoji) {
                    // the emoji goes where the command was; the space before it stays
                    view.dispatch({ changes: { from: f, to: t, insert: '' } })
                    onemoji(f)
                    return
                  }
                  if (c.log) {
                    const r = logChanges(view.state.doc.toString(), f, t, logStamp())
                    view.dispatch({ changes: r.changes, selection: { anchor: r.cursor } })
                    return
                  }
                  if (c.newProperty) {
                    // a fresh `::` line under the block's properties; the key menu opens on it
                    const cut = view.state.sliceDoc(f - 1, f) === ' ' ? f - 1 : f
                    view.dispatch({ changes: { from: cut, to: t, insert: '' } })
                    const end = propertyEnd(view.state.doc.toString())
                    view.dispatch({
                      changes: { from: end, to: end, insert: '\n::' },
                      selection: { anchor: end + 3 },
                    })
                    startCompletion(view)
                    return
                  }
                  if (c.property) {
                    // the property line goes under the block's first line, wherever the command was typed
                    const doc = view.state.doc.toString()
                    const cut = view.state.sliceDoc(f - 1, f) === ' ' ? f - 1 : f
                    const set = propertyChanges(doc, c.property.key, c.property.value)
                    view.dispatch({
                      changes: [{ from: cut, to: t, insert: '' }, ...set].sort((x, y) => x.from - y.from),
                    })
                    return
                  }
                  if (c.date) {
                    // property commands go on their own line, so drop the space typed before the command
                    const cut = c.date !== 'link' && view.state.sliceDoc(f - 1, f) === ' ' ? f - 1 : f
                    view.dispatch({ changes: { from: cut, to: t, insert: '' } })
                    onpickdate(c.date, cut)
                    return
                  }
                  const out = c.insert()
                  view.dispatch({
                    changes: { from: f, to: t, insert: out.text },
                    selection: { anchor: f + (out.cursor ?? out.text.length) },
                  })
                },
              })),
            }
          }
          return null
        },
      ],
    }),
    menuTheme,
  ]
}
