import { backend, CommandError, type GraphInfo } from '../api'
import { parseRoute, routeHash, type Route } from '../route'
import { setDateFormat, setDateLocale } from '../dates'
import { loadIcons } from './icons.svelte'
import { useRecentFor } from './recent.svelte'

/** What the "To another graph" dialog was opened for. */
export interface TransferRequest {
  kind: 'pages' | 'blocks'
  page: string
  /** `blocks`: the paths of the selected blocks. */
  blocks?: number[][]
  /** `pages`: the page has pages below it. */
  hasSubpages?: boolean
}

export interface Toast {
  id: number
  kind: 'info' | 'error'
  message: string
  /** A button on the toast (Undo); a toast with an action stays longer. */
  action?: { label: string; run: () => void }
}

export type Theme = 'system' | 'light' | 'dark'

/** App-wide reactive state (Svelte 5 runes in a .svelte.ts module: import `app` anywhere and read/write its fields). */
const LEFT_MIN = 180
const LEFT_MAX = 560
export const LEFT_DEFAULT = 240

function loadLeftWidth(): number {
  try {
    const n = Number(localStorage.getItem('notatnik-left-width'))
    return Number.isFinite(n) && n >= LEFT_MIN && n <= LEFT_MAX ? n : LEFT_DEFAULT
  } catch {
    return LEFT_DEFAULT
  }
}

function loadLeftOpen(): boolean {
  try {
    return localStorage.getItem('notatnik-left-open') !== 'no'
  } catch {
    return true
  }
}

const RIGHT_MIN = 280
export const RIGHT_DEFAULT = 380
/** The window is wide enough for a side panel next to the page; below this the panels fall back to the bottom. */
const WIDE_PX = 1180

function loadRight(): { open: boolean; width: number } {
  try {
    const n = Number(localStorage.getItem('notatnik-right-width'))
    return {
      open: localStorage.getItem('notatnik-right-open') !== 'no',
      width: Number.isFinite(n) && n >= RIGHT_MIN ? n : RIGHT_DEFAULT,
    }
  } catch {
    return { open: true, width: RIGHT_DEFAULT }
  }
}

export type Place = 'bottom' | 'side'
/** Where the references and the tasks of a page are shown (device setting). */
export type Placement = { refs: Place; tasks: Place }

function loadPlacement(): Placement {
  try {
    const v = JSON.parse(localStorage.getItem('notatnik-placement') ?? '{}')
    return { refs: v.refs === 'side' ? 'side' : 'bottom', tasks: v.tasks === 'side' ? 'side' : 'bottom' }
  } catch {
    return { refs: 'bottom', tasks: 'bottom' }
  }
}

const wideQuery =
  typeof window.matchMedia === 'function' ? window.matchMedia(`(min-width: ${WIDE_PX}px)`) : null

export const app = $state({
  /** Width of the right panel in px, and whether it is shown; remembered on this device. */
  rightWidth: loadRight().width,
  rightOpen: loadRight().open,
  /** The right panel fills the window and the page is folded away to a strip (not remembered across restarts). */
  rightFocus: false,
  /** The tab of the right panel that is open: `refs`, `tasks` or `pages` (the pages opened beside the page). */
  rightTab: (localStorage.getItem('notatnik-right-tab') ?? 'refs') as 'refs' | 'tasks' | 'pages',
  /** Where the references and the tasks of a page go. */
  placement: loadPlacement(),
  /** The window is wide enough for a side panel. */
  wide: wideQuery?.matches ?? true,
  graph: null as GraphInfo | null,
  needsGraph: false,
  today: '',
  route: parseRoute(window.location.hash) as Route,
  toasts: [] as Toast[],
  theme: 'system' as Theme,
  conflicts: [] as string[],
  rightPanels: [] as string[],
  paletteOpen: false,
  searchOpen: false,
  /** A page that was just created from the dialog: its first block opens for typing. */
  draftFor: '',
  /** Blocks (Markdown) waiting for a name in the "Save as template" dialog; `null` when it is closed. */
  snippetText: null as string | null,
  /** Notes waiting in the "To another graph" dialog (MG-2); `null` when it is closed. */
  transfer: null as TransferRequest | null,
  /** The questions of a template being inserted (`{{ask:Label}}`), while the dialog waits for the answers. */
  templateAsks: null as { name: string; asks: { label: string; default: string }[] } | null,
  /** The page whose Hierarchy should open its "add subpage" field (set by the palette command). */
  subpageFor: '',
  /** The left sidebar is shown; remembered on this device. */
  leftOpen: loadLeftOpen(),
  /** Width of the left sidebar in px; remembered on this device. */
  leftWidth: loadLeftWidth(),
  /** Bumped when saved queries change, so the sidebar reloads them. */
  savedVersion: 0,
  /** Bumped when the date language/format change, so shown dates re-render (see `shownDate`). */
  dateVersion: 0,
  /** Device setting: a pasted web address gets the page's title as link text (the app contacts the site). */
  pasteTitles: false,
  /** Device setting: `compact` shows a task's schedule, repeat and logbook as small indicators (details on hover). */
  taskDetails: 'full' as 'full' | 'compact',
  /** Device setting: `compact` shows a block's properties as one small indicator (the card opens on hover). */
  blockProps: 'full' as 'full' | 'compact',
})

/** Sets the sidebar width in px (clamped); `save` remembers it on this device (done when a drag ends). */
export function setLeftWidth(px: number, save = true) {
  app.leftWidth = Math.round(Math.min(LEFT_MAX, Math.max(LEFT_MIN, px)))
  if (!save) return
  try {
    localStorage.setItem('notatnik-left-width', String(app.leftWidth))
  } catch {
    /* not remembered */
  }
}

wideQuery?.addEventListener('change', (e) => {
  app.wide = e.matches
})

/** A page's `refs` (linked and unlinked references) or `tasks` are in the right panel: chosen so, and room for it. */
export function onSide(kind: keyof Placement): boolean {
  return app.wide && app.placement[kind] === 'side'
}

export function setPlacement(kind: keyof Placement, place: Place) {
  app.placement[kind] = place
  try {
    localStorage.setItem('notatnik-placement', JSON.stringify(app.placement))
  } catch {
    /* not remembered */
  }
}

/** Sets the right panel's width in px (clamped); `save` remembers it (done when a drag ends). */
export function setRightWidth(px: number, save = true) {
  app.rightWidth = Math.round(Math.max(RIGHT_MIN, px))
  if (!save) return
  try {
    localStorage.setItem('notatnik-right-width', String(app.rightWidth))
  } catch {
    /* not remembered */
  }
}

export function setRightTab(tab: 'refs' | 'tasks' | 'pages') {
  app.rightTab = tab
  try {
    localStorage.setItem('notatnik-right-tab', tab)
  } catch {
    /* not remembered */
  }
}

/** The page is folded away and the right panel gets the whole window, or the page is back. */
export function toggleRightFocus() {
  app.rightFocus = !app.rightFocus
}

/** The least width the page keeps beside the panel before it is folded away. */
export const PAGE_MIN = 320

/** Shows or hides the right panel (Ctrl+Shift+\\ and the button in the corner). */
export function toggleRight() {
  app.rightFocus = false
  app.rightOpen = !app.rightOpen
  try {
    localStorage.setItem('notatnik-right-open', app.rightOpen ? 'yes' : 'no')
  } catch {
    /* not remembered */
  }
}

/** Shows or hides the left sidebar (Ctrl+\\ and the button beside it). */
export function toggleLeft() {
  app.leftOpen = !app.leftOpen
  try {
    localStorage.setItem('notatnik-left-open', app.leftOpen ? 'yes' : 'no')
  } catch {
    /* not remembered */
  }
}

let toastId = 1
export function toast(message: string, kind: Toast['kind'] = 'info', action?: Toast['action']) {
  const t: Toast = { id: toastId++, kind, message, action }
  app.toasts.push(t)
  setTimeout(
    () => {
      app.toasts = app.toasts.filter((x) => x.id !== t.id)
    },
    kind === 'error' || action ? 10000 : 3500,
  )
}

export function dismissToast(id: number) {
  app.toasts = app.toasts.filter((x) => x.id !== id)
}

export function errorMessage(e: unknown): string {
  return e instanceof CommandError || e instanceof Error ? e.message : String(e)
}

export function navigate(r: Route) {
  const h = routeHash(r)
  if (window.location.hash !== h) window.location.hash = h
  else app.route = r
}

export function applyTheme(theme: Theme) {
  app.theme = theme
  const dark =
    theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
  document.documentElement.dataset.theme = dark ? 'dark' : 'light'
  try {
    localStorage.setItem('notatnik-theme', theme)
  } catch {
    /* the preference just isn't remembered */
  }
}

export async function refreshConflicts() {
  try {
    app.conflicts = (await backend.call('list_conflicts', {})).items
  } catch {
    app.conflicts = []
  }
}

export async function bootstrap() {
  window.addEventListener('hashchange', () => {
    app.route = parseRoute(window.location.hash)
  })
  let theme: Theme = 'system'
  try {
    theme = (localStorage.getItem('notatnik-theme') as Theme | null) ?? 'system'
  } catch {
    /* default */
  }
  applyTheme(theme)
  try {
    app.graph = await backend.call('graph_info', {})
    useRecentFor(app.graph.id)
    app.today = (await backend.call('today', {})).value
    await refreshConflicts()
    await loadDeviceSettings()
    void loadIcons()
  } catch (e) {
    if (e instanceof CommandError && e.code === 'no_graph') app.needsGraph = true
    else toast(errorMessage(e), 'error')
  }
}

/** Rendered links show `[[ ]]` only when this device setting is on (the CSS reads the attribute). */
export function applyLinkBrackets(show: boolean) {
  document.documentElement.dataset.linkBrackets = show ? 'show' : 'hide'
}

/** How page icons are drawn: the CSS reads the attribute (`color`, `muted`, `mono`). */
export function applyIconStyle(style: string) {
  document.documentElement.dataset.iconStyle = style === 'color' || style === 'mono' ? style : 'muted'
}

/** Theme and date format come from the device settings file. */
export async function loadDeviceSettings() {
  try {
    const s = await backend.call('get_settings', {})
    setDateLocale(s.date_locale)
    setDateFormat(s.date_format, s.date_pattern)
    app.dateVersion++
    applyTheme(s.theme as Theme)
    applyLinkBrackets(s.link_brackets)
    applyIconStyle(s.icon_style)
    app.pasteTitles = s.paste_titles
    app.taskDetails = s.task_details === 'compact' ? 'compact' : 'full'
    app.blockProps = s.block_properties === 'compact' ? 'compact' : 'full'
  } catch {
    /* keep the defaults */
  }
}

export async function openGraph(path: string, create: boolean) {
  try {
    await adoptGraph(await backend.call('open_graph', { path, create }))
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
}

/** Makes `info` (a graph the backend has just opened) the graph shown. Used after `open_graph` and after
 * `create_remote_graph`, which opens the new graph itself. */
export async function adoptGraph(info: GraphInfo) {
  if (app.graph) {
    // another graph: nothing of the pages loaded so far applies, so start the page over. Until the reload
    // finishes the screen stays as it is, so nothing of the new graph is shown with the old pages.
    window.location.hash = '#/journals'
    window.location.reload()
    return
  }
  app.graph = info
  app.needsGraph = false
  useRecentFor(info.id)
  app.today = (await backend.call('today', {})).value
  await refreshConflicts()
  await loadDeviceSettings()
  void loadIcons()
}

backend.onGraphChanged(() => {
  void refreshConflicts()
})
