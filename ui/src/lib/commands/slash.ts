// Slash commands in the editor (`/today`, `/todo`, ...). Dates come from the server's clock (injectable in tests).
import { backend } from '../api'
import { app } from '../stores/app.svelte'
import { parseNaturalDate } from '../dates'

export type SlashGroup = 'Tasks' | 'Dates' | 'Task details' | 'Blocks' | 'Insert'
/** Menu order of the groups. */
export const SLASH_GROUPS: SlashGroup[] = ['Tasks', 'Dates', 'Task details', 'Blocks', 'Insert']

export interface SlashCommand {
  /** What you type after the `/`. */
  name: string
  /** One-line description shown under the label. */
  title: string
  /** Short display name in the menu. */
  label: string
  /** A single glyph for the icon tile. */
  icon: string
  group: SlashGroup
  /** Extra words that find the command (`/task` finds `/todo`). */
  keywords: string[]
  insert: () => { text: string; cursor?: number }
  /** Task commands set the marker at the start of the block instead of inserting text at the cursor. */
  marker?: string
  /** Date commands open the date picker; the picked date is applied with `dateChanges`. */
  date?: DateKind
  /** `/priority`: sets a `key:: value` property line of the block, wherever the command was typed (see `setProperty`). */
  property?: { key: string; value: string | null }
  /** `/property` starts a new `key::` line under the block's other properties and opens the key menu. */
  newProperty?: boolean
  /** `/log` adds a dated line to the block's `:LOGBOOK:` drawer (see `logChanges`). */
  log?: boolean
  /** `/emoji` opens the emoji picker under the cursor. */
  emoji?: boolean
  /** `/file` opens the file picker: a server, this computer or an address; the link goes where the command was. */
  file?: boolean
  /** `/followup`: marks the task done and creates the next one, linked with `follows::`. */
  followup?: boolean
  /** `/template-<name>` inserts the blocks of this template after the block: a page type, or `snippets/<name>`
   * for blocks saved from a page. */
  template?: string
}

// The templates of the graph, for `/template`. Refreshed whenever an editor opens (a block is being edited before
// a command can be typed), so a template saved a moment ago is offered.
let templateNames: string[] = []
let snippetNames: string[] = []
export async function refreshTemplateNames(): Promise<void> {
  try {
    templateNames = (await backend.call('list_templates', {})).map((t) => t.name)
    snippetNames = (await backend.call('list_snippets', {})).map((t) => t.name)
  } catch {
    templateNames = []
    snippetNames = []
  }
}
/** For tests. */
export function setTemplateNames(names: string[], snippets: string[] = []): void {
  templateNames = names
  snippetNames = snippets
}

const nameSlug = (n: string) =>
  n
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '') || 'x'

function templateCommands(): SlashCommand[] {
  return [
    ...templateNames.map((n) => ({
      name: `template-${nameSlug(n)}`,
      label: `Template: ${n}`,
      icon: '⎘',
      group: 'Insert' as const,
      keywords: ['template', n.toLowerCase()],
      title: `Insert the blocks of the template “${n}”`,
      template: n,
      insert: () => ({ text: '' }),
    })),
    // blocks saved from a page (Save as template…): no page type, only blocks
    ...snippetNames.map((n) => ({
      name: `template-snippet-${nameSlug(n)}`,
      label: `Snippet: ${n}`,
      icon: '⎘',
      group: 'Insert' as const,
      keywords: ['template', 'snippet', n.toLowerCase()],
      title: `Insert the saved blocks “${n}”`,
      template: `snippets/${n}`,
      insert: () => ({ text: '' }),
    })),
  ]
}

/** `YYYY-MM-DD HH:MM` for a new logbook line: the app's date and the local time of day. */
export function logStamp(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  const date = app.today || now.toISOString().slice(0, 10)
  return `${date} ${pad(now.getHours())}:${pad(now.getMinutes())}`
}

/** Changes for `/log` typed at `from..to`: removes the command, then adds `stamp` as a new line of the
 * block's `:LOGBOOK:` drawer (creating the drawer at the end of the block if there is none). `cursor` is
 * where to continue typing the note (in the new document). */
export function logChanges(
  doc: string,
  from: number,
  to: number,
  stamp: string,
): { changes: { from: number; to: number; insert: string }[]; cursor: number } {
  let a = from
  const b = to
  if (a > 0 && doc[a - 1] === ' ' && (b >= doc.length || /\s/.test(doc[b]))) a--
  const cut = { from: a, to: b, insert: '' }
  const start = /^[ \t]*:LOGBOOK:[ \t]*$/m.exec(doc)
  const afterStart = start ? start.index + start[0].length : -1
  const end = start ? /^[ \t]*:END:[ \t]*$/m.exec(doc.slice(afterStart)) : null
  const line = `${stamp} `
  if (start && end) {
    const at = afterStart + end.index
    const shift = b <= at ? b - a : 0
    return {
      changes: [cut, { from: at, to: at, insert: `${line}\n` }].sort((x, y) => x.from - y.from),
      cursor: at - shift + line.length,
    }
  }
  const head = '\n:LOGBOOK:\n'
  return {
    changes: [cut, { from: doc.length, to: doc.length, insert: `${head}${line}\n:END:` }],
    cursor: doc.length - (b - a) + head.length + line.length,
  }
}

export type DateKind = 'link' | 'scheduled' | 'deadline'

export const DATE_TITLES: Record<DateKind, string> = {
  link: 'Insert a date',
  scheduled: 'Scheduled date',
  deadline: 'Deadline',
}

/** Changes that apply a picked date. `link` inserts `[[date]]` at `at`; `scheduled`/`deadline` set the property
 * line (replacing an existing one, else adding a new line under the first line, after its other properties). */
export function dateChanges(
  doc: string,
  at: number,
  kind: DateKind,
  iso: string,
): { from: number; to: number; insert: string }[] {
  if (kind === 'link') return [{ from: at, to: at, insert: `[[${iso}]]` }]
  return propertyChanges(doc, kind, iso)
}

/** Changes that set the property `key` of a block: replace the value of an existing `key:: value` line, else add
 * one. Properties only count directly under the first line, so a new line goes after the properties already
 * there and before everything else (a `:LOGBOOK:` drawer, a body line), never at the end of the block. */
export function propertyChanges(
  doc: string,
  key: string,
  value: string | null,
): { from: number; to: number; insert: string }[] {
  const existing = new RegExp(`^([ \\t]*${key}:: ?)(.*)$`, 'm').exec(doc)
  if (value === null) {
    // remove the line, with the line break before it
    if (!existing) return []
    const from = existing.index > 0 ? existing.index - 1 : existing.index
    return [{ from, to: existing.index + existing[0].length, insert: '' }]
  }
  if (existing) {
    const from = existing.index + existing[1].length
    return [{ from, to: existing.index + existing[0].length, insert: value }]
  }
  const end = propertyEnd(doc)
  return [{ from: end, to: end, insert: `\n${key}:: ${value}` }]
}

/** Offset where a new property line goes: the end of the first line, or of the last `key:: value` line that
 * follows it directly. */
export function propertyEnd(doc: string): number {
  let end = doc.indexOf('\n')
  if (end < 0) return doc.length
  for (;;) {
    const next = doc.indexOf('\n', end + 1)
    const line = doc.slice(end + 1, next < 0 ? doc.length : next)
    if (!/^[ \t]*[^\s:]+:: ?/.test(line)) return end
    end = next < 0 ? doc.length : next
    if (next < 0) return end
  }
}

const MARKER = /^(?:TODO|LATER|DOING|NOW|WAITING|DONE|CANCELED|\[[ xX]\])[ \t]+/

/** Changes that remove the typed `/command` (`from`..`to`) and put `marker` at the start of the block,
 * replacing an existing one. The command may be typed anywhere in the block. */
export function markerChanges(
  doc: string,
  from: number,
  to: number,
  marker: string,
): { from: number; to: number; insert: string }[] {
  // remove one adjacent space too, so `buy milk /todo` and `/todo buy milk` leave no gap
  let a = from
  let b = to
  if (a > 0 && doc[a - 1] === ' ') a--
  else if (doc[b] === ' ') b++
  const headEnd = MARKER.exec(doc)?.[0].length ?? 0
  if (a > headEnd) {
    return [
      { from: 0, to: headEnd, insert: `${marker} ` },
      { from: a, to: b, insert: '' },
    ]
  }
  // the command touches the old marker (`DOING /waiting x`): replace everything up to the command's end
  const rest = doc.slice(b)
  const gap = rest === '' || /^\s/.test(rest) ? '' : ' '
  return [{ from: 0, to: Math.max(b, headEnd), insert: marker + gap }]
}

const link = (d: string) => `[[${d}]]`

/** Orders commands for what was typed after the `/`: name prefix, then keyword prefix, then label or
 * description contains it. Nothing typed keeps the menu order. Pure, so it is unit-tested. */
export function rankCommands(cmds: SlashCommand[], typed: string): SlashCommand[] {
  const q = typed.trim().toLowerCase()
  if (!q) return cmds
  const score = (c: SlashCommand): number => {
    if (c.name === q) return 0
    if (c.name.startsWith(q)) return 1
    if (c.keywords.some((k) => k.startsWith(q))) return 2
    if (c.label.toLowerCase().includes(q) || c.title.toLowerCase().includes(q)) return 3
    if (c.keywords.some((k) => k.includes(q))) return 4
    return 9
  }
  return cmds
    .map((c, i) => ({ c, i, s: score(c) }))
    .filter((x) => x.s < 9)
    .sort((a, b) => a.s - b.s || a.i - b.i)
    .map((x) => x.c)
}

/** A name typed after the `/`: letters, digits and hyphens only (`P1` → `p1`). */
function slug(label: string, i: number): string {
  return label.toLowerCase().replace(/[^a-z0-9]/g, '') || String(i + 1)
}

function priorityCommands(): SlashCommand[] {
  const labels = app.graph?.priority_labels?.length ? app.graph.priority_labels : ['A', 'B', 'C']
  const none = () => ({ text: '' })
  return [
    ...labels.map((label, i) => ({
      // `/priority` is the most important one; the others are `/priority-b`, ...
      name: i === 0 ? 'priority' : `priority-${slug(label, i)}`,
      label: `Priority ${label}`,
      icon: '⚑',
      group: 'Task details' as const,
      keywords: ['important', 'urgent', 'priority', label.toLowerCase(), `p${i + 1}`],
      title: `Set the priority to [#${label}]${i === 0 ? ' (the most important)' : ''}`,
      property: { key: 'priority', value: label },
      insert: none,
    })),
    {
      name: 'priority-none',
      label: 'No priority',
      icon: '⚐',
      group: 'Task details' as const,
      keywords: ['clear', 'remove', 'priority'],
      title: 'Remove the priority',
      property: { key: 'priority', value: null },
      insert: none,
    },
  ]
}

/** Where a `/command` is being typed. `midWord`: right after a letter or digit (`wor/d`), where a command may
 * only change the block's metadata (marker, logbook, dates, priority), never insert text into the word.
 * URLs, paths and fractions (`https://x.y/z`, `a/b/c`, `1/2`) are left alone. */
export function slashAt(before: string): { typed: string; midWord: boolean } | null {
  // after whitespace or punctuation (`buy milk /todo`, `(/todo`), not after a letter, a `/`, `:` or `.`
  const word = /(?:^|[^\p{L}\p{N}/:.\\@])\/([a-z0-9-]*)$/u.exec(before)
  if (word) return { typed: word[1], midWord: false }
  // the whole token before the cursor: letters, digits and punctuation, one slash, then the command so far
  const tok = /(\S+)$/.exec(before)?.[1] ?? ''
  const m = /^[^\s/:.\\@]*[\p{L}\p{N}]\/([a-z0-9-]+)$/u.exec(tok)
  if (m) return { typed: m[1], midWord: true }
  return null
}

/** The commands offered for a `/command` typed at `at`. In the middle of a word only metadata commands, and only
 * those whose name or keyword *starts* with what was typed (so `km/h` or `and/or` stay quiet). */
export function slashPool(at: { typed: string; midWord: boolean }): SlashCommand[] {
  const all = slashCommands()
  if (!at.midWord) return all
  return all
    .filter(isMetadataCommand)
    .filter((c) => c.name.startsWith(at.typed) || c.keywords.some((k) => k.startsWith(at.typed)))
}

/** Commands that only change metadata: usable in the middle of a word. */
export const isMetadataCommand = (c: SlashCommand): boolean =>
  !!(c.marker || c.log || c.property || c.newProperty || (c.date && c.date !== 'link'))

export const slashCommands = (): SlashCommand[] => [...baseCommands(), ...templateCommands()]

function baseCommands(): SlashCommand[] {
  const today = app.today || new Date().toISOString().slice(0, 10)
  const day = (phrase: string) => parseNaturalDate(phrase, today) ?? today
  const none = () => ({ text: '' })
  return [
    {
      name: 'todo',
      label: 'To do',
      icon: '☐',
      group: 'Tasks',
      keywords: ['task', 'checkbox'],
      title: 'Make this a task',
      marker: 'TODO',
      insert: none,
    },
    {
      name: 'doing',
      label: 'Doing',
      icon: '◐',
      group: 'Tasks',
      keywords: ['progress', 'started'],
      title: 'Mark as doing',
      marker: 'DOING',
      insert: none,
    },
    {
      name: 'later',
      label: 'Later',
      icon: '☐',
      group: 'Tasks',
      keywords: ['task', 'someday'],
      title: 'Make this a task to do later (LATER, the same step as TODO)',
      marker: 'LATER',
      insert: none,
    },
    {
      name: 'now',
      label: 'Now',
      icon: '◐',
      group: 'Tasks',
      keywords: ['progress', 'started'],
      title: 'Mark as in progress (NOW, the same step as DOING)',
      marker: 'NOW',
      insert: none,
    },
    {
      name: 'waiting',
      label: 'Waiting',
      icon: '◔',
      group: 'Tasks',
      keywords: ['blocked'],
      title: 'Mark as waiting',
      marker: 'WAITING',
      insert: none,
    },
    {
      name: 'done',
      label: 'Done',
      icon: '✓',
      group: 'Tasks',
      keywords: ['complete', 'finished'],
      title: 'Mark as done',
      marker: 'DONE',
      insert: none,
    },
    {
      name: 'followup',
      label: 'Done + follow-up',
      icon: '⤷',
      group: 'Tasks',
      keywords: ['done-next', 'next', 'follow', 'follow-up', 'chain', 'finish'],
      title: 'Mark this task done and create the next one, linked to it',
      followup: true,
      insert: none,
    },
    {
      name: 'canceled',
      label: 'Canceled',
      icon: '✕',
      group: 'Tasks',
      keywords: ['cancelled', 'dropped'],
      title: 'Mark as canceled',
      marker: 'CANCELED',
      insert: none,
    },
    {
      name: 'today',
      label: 'Today',
      icon: '☀',
      group: 'Dates',
      keywords: ['date', 'now'],
      title: "Insert today's date",
      insert: () => ({ text: link(day('today')) }),
    },
    {
      name: 'tomorrow',
      label: 'Tomorrow',
      icon: '→',
      group: 'Dates',
      keywords: ['date', 'next'],
      title: "Insert tomorrow's date",
      insert: () => ({ text: link(day('tomorrow')) }),
    },
    {
      name: 'yesterday',
      label: 'Yesterday',
      icon: '←',
      group: 'Dates',
      keywords: ['date', 'previous'],
      title: "Insert yesterday's date",
      insert: () => ({ text: link(day('yesterday')) }),
    },
    {
      name: 'date',
      label: 'Pick a date…',
      icon: '▦',
      group: 'Dates',
      keywords: ['calendar', 'link'],
      title: 'Pick a date',
      date: 'link',
      insert: none,
    },
    {
      name: 'scheduled',
      label: 'Scheduled',
      icon: '⏳',
      group: 'Task details',
      keywords: ['start', 'plan', 'when'],
      title: 'Set scheduled date',
      date: 'scheduled',
      insert: none,
    },
    {
      name: 'deadline',
      label: 'Deadline',
      icon: '📅',
      group: 'Task details',
      keywords: ['due', 'date'],
      title: 'Set deadline',
      date: 'deadline',
      insert: none,
    },
    // one command per priority label of the graph (`[tasks] priorities`, A, B, C by default), and one to clear it
    ...priorityCommands(),
    {
      name: 'property',
      label: 'Property…',
      icon: '::',
      group: 'Blocks',
      keywords: ['attribute', 'field', 'key', 'value', 'metadata'],
      title: 'Add a property (key:: value)',
      newProperty: true,
      insert: none,
    },
    {
      name: 'log',
      label: 'Log progress',
      icon: '✎',
      group: 'Task details',
      keywords: ['logbook', 'note', 'journal', 'update'],
      title: 'Log progress (dated note)',
      log: true,
      insert: none,
    },
    {
      name: 'emoji',
      label: 'Emoji…',
      icon: '😀',
      group: 'Insert',
      keywords: ['smiley', 'icon', 'face', 'symbol'],
      title: 'Pick an emoji',
      emoji: true,
      insert: none,
    },
    {
      name: 'file',
      label: 'Link a file or folder…',
      icon: '📎',
      group: 'Insert',
      keywords: [
        'link',
        'attach',
        'attachment',
        'nextcloud',
        'webdav',
        'server',
        'upload',
        'document',
        'pdf',
        'folder',
        'directory',
      ],
      title: 'Link a file or folder from a server, this computer or an address',
      file: true,
      insert: none,
    },
    {
      name: 'code',
      label: 'Code block',
      icon: '</>',
      group: 'Blocks',
      keywords: ['fence', 'snippet'],
      title: 'Code block',
      insert: () => ({ text: '```\n\n```', cursor: 4 }),
    },
    {
      name: 'query',
      label: 'Query',
      icon: '⌕',
      group: 'Blocks',
      keywords: ['search', 'filter', 'view', 'list'],
      title: 'Query block',
      insert: () => ({ text: '```query\ntask: todo\n```', cursor: 9 }),
    },
  ]
}
