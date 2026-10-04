import { backend, CommandError, type GraphInfo } from '../api'
import { parseRoute, routeHash, type Route } from '../route'

export interface Toast {
  id: number
  kind: 'info' | 'error'
  message: string
}

export type Theme = 'system' | 'light' | 'dark'

/** App-wide reactive state (Svelte 5 runes in a .svelte.ts module: import `app` anywhere and read/write its fields). */
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
  leftOpen: true,
  /** Bumped when saved queries change, so the sidebar reloads them. */
  savedVersion: 0,
})

let toastId = 1
export function toast(message: string, kind: Toast['kind'] = 'info') {
  const t = { id: toastId++, kind, message }
  app.toasts.push(t)
  setTimeout(
    () => {
      app.toasts = app.toasts.filter((x) => x.id !== t.id)
    },
    kind === 'error' ? 8000 : 3500,
  )
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
    app.today = (await backend.call('today', {})).value
    await refreshConflicts()
  } catch (e) {
    if (e instanceof CommandError && e.code === 'no_graph') app.needsGraph = true
    else toast(errorMessage(e), 'error')
  }
}

export async function openGraph(path: string, create: boolean) {
  try {
    app.graph = await backend.call('open_graph', { path, create })
    app.needsGraph = false
    app.today = (await backend.call('today', {})).value
    await refreshConflicts()
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
}

backend.onGraphChanged(() => {
  void refreshConflicts()
})
