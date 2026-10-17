import { backend, CommandError, type GraphInfo } from '../api'
import { parseRoute, routeHash, type Route } from '../route'
import { setDateFormat, setDateLocale } from '../dates'
import { loadIcons } from './icons.svelte'
import { useRecentFor } from './recent.svelte'

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

export const app = $state({
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
  /** The left sidebar is shown; remembered on this device. */
  leftOpen: loadLeftOpen(),
  /** Width of the left sidebar in px; remembered on this device. */
  leftWidth: loadLeftWidth(),
  /** Bumped when saved queries change, so the sidebar reloads them. */
  savedVersion: 0,
  /** Bumped when the date language/format change, so shown dates re-render (see `shownDate`). */
  dateVersion: 0,
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

/** Theme and date format come from the device settings file. */
export async function loadDeviceSettings() {
  try {
    const s = await backend.call('get_settings', {})
    setDateLocale(s.date_locale)
    setDateFormat(s.date_format, s.date_pattern)
    app.dateVersion++
    applyTheme(s.theme as Theme)
    applyLinkBrackets(s.link_brackets)
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
  const switching = !!app.graph
  app.graph = info
  app.needsGraph = false
  if (switching) {
    // another graph: nothing of the pages loaded so far applies, so start the page over
    window.location.hash = '#/journals'
    window.location.reload()
    return
  }
  useRecentFor(info.id)
  app.today = (await backend.call('today', {})).value
  await refreshConflicts()
  await loadDeviceSettings()
  void loadIcons()
}

backend.onGraphChanged(() => {
  void refreshConflicts()
})
