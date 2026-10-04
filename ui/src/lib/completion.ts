// Autocomplete in the block editor: [[page, ((block, #tag and /commands (ED-7, NS-7).
import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import type { Extension } from '@codemirror/state'
import { backend, type TitleHit } from './api'
import { slashCommands } from './commands/slash'

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

export function wikiCompletions(pageName: () => string): Extension {
  return autocompletion({
    activateOnTyping: true,
    closeOnBlur: false,
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
          const hits = (await backend.call('search', { text: q, limit: 10 })).filter(
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
        // /command at the start of a word
        m = /(?:^|\s)\/([a-z]*)$/.exec(before)
        if (m) {
          const typed = m[1]
          return {
            from: ctx.pos - typed.length - 1,
            filter: true,
            options: slashCommands().map((c) => ({
              label: '/' + c.name,
              detail: c.title,
              type: 'function',
              apply: (view, _comp, f, t) => {
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
  })
}
