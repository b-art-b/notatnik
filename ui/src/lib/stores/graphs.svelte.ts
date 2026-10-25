// The graphs that are open (MG-1, MG-6): the first graph (slot `''`) and, when the person opened one, a second (slot
// `'1'`). Each has a `GraphState`; one of them is *active*: the page area shows its pages, and `backend`, `recent`,
// `favorites` ... answer for it. The hash says which: `#/g/1/page/X` is in the second graph.
import { untrack } from 'svelte'
import { backend, backendFor, CommandError, setActiveSlot, workspace, type GraphInfo } from '../api'
import { parseHash, routeHash, setRouteSlot, type Route } from '../route'
import { active, setActive } from './active.svelte'
import {
  app,
  applyTheme,
  errorMessage,
  loadDeviceSettings,
  refreshConflicts,
  toast,
  type Theme,
} from './app.svelte'
import { stopEditing } from './editing.svelte'
import { GraphState } from './graphState.svelte'
import { flushAll, resetDocs } from './pages.svelte'

/** The slot of the second graph. */
export const SECOND = '1'
const REMEMBERED = 'notatnik-second-graph'

/** The open graphs, the first graph first. */
export const graphs = $state({ list: [] as GraphState[] })

export const stateFor = (slot: string): GraphState | undefined => graphs.list.find((g) => g.slot === slot)

/** Have two graphs open: pages then say which graph they belong to. */
export const twoOpen = () => graphs.list.length > 1

function mount(slot: string, info: GraphInfo): GraphState {
  const old = stateFor(slot)
  old?.dispose()
  const g = new GraphState(slot, info)
  graphs.list = [...graphs.list.filter((x) => x.slot !== slot), g].sort((a, b) =>
    a.slot.localeCompare(b.slot),
  )
  g.start()
  return g
}

/** Makes the graph in `slot` the one the page area shows. The pages open so far belong to the other graph: they are
 * saved first and dropped. */
export async function activate(slot: string, route: Route = { kind: 'journals' }): Promise<void> {
  const g = stateFor(slot) ?? stateFor('')
  if (!g) return
  if (active() === g) {
    app.route = route
    return
  }
  await flushAll()
  untrack(() => {
    resetDocs()
    stopEditing()
    app.rightPanels = []
    app.transfer = null
    setActiveSlot(g.slot)
    setRouteSlot(g.slot)
    setActive(g)
    app.graph = g.info
    // the route is set together with the graph: the page of the other graph is never shown in this one
    app.route = route
  })
  void refreshConflicts()
}

async function onHashChange() {
  const { slot, route } = parseHash(window.location.hash)
  if (slot !== active().slot && stateFor(slot)) await activate(slot, route)
  else app.route = route
}

/** Reads `graph_info` again for the active graph (a setting changed), or takes `info` that a command returned. */
export async function refreshActiveInfo(info?: GraphInfo): Promise<void> {
  const g = active()
  try {
    g.info = info ?? (await g.backend.call('graph_info', {}))
    app.graph = g.info
  } catch {
    /* the graph was closed meanwhile */
  }
}

export async function bootstrap() {
  window.addEventListener('hashchange', () => void onHashChange())
  let theme: Theme = 'system'
  try {
    theme = (localStorage.getItem('notatnik-theme') as Theme | null) ?? 'system'
  } catch {
    /* default */
  }
  applyTheme(theme)
  try {
    const slots = await workspace.call('slots', {})
    if (!slots.some((s) => s.slot === ''))
      throw new CommandError({ code: 'no_graph', message: 'no graph is open' })
    for (const s of slots) mount(s.slot, await backendFor(s.slot).call('graph_info', {}))
    const wanted = parseHash(window.location.hash).slot
    await activate(stateFor(wanted) ? wanted : '', parseHash(window.location.hash).route)
    app.today = (await backend.call('today', {})).value
    await loadDeviceSettings()
    void refreshConflicts()
    void restoreSecond()
  } catch (e) {
    if (e instanceof CommandError && e.code === 'no_graph') app.needsGraph = true
    else toast(errorMessage(e), 'error')
  }
}

/** The second graph of the last time is opened again (the backend forgets it when the app quits). */
async function restoreSecond() {
  if (stateFor(SECOND)) return
  try {
    const path = localStorage.getItem(REMEMBERED)
    if (path) await openSecond(path, true)
  } catch {
    /* nothing remembered */
  }
}

/** Opens a graph as the second one (the graph there so far is closed). `quiet`: no message when it cannot be opened. */
export async function openSecond(path: string, quiet = false): Promise<void> {
  try {
    const info = await backendFor(SECOND).call('open_graph', { path, create: false })
    mount(SECOND, info)
    try {
      localStorage.setItem(REMEMBERED, path)
    } catch {
      /* not remembered */
    }
  } catch (e) {
    if (quiet) return
    toast(
      e instanceof CommandError && e.code === 'already_open'
        ? 'That graph is open already.'
        : errorMessage(e),
      'error',
    )
  }
}

/** Closes the second graph; if its pages are on screen, the first graph's journals are. */
export async function closeSecond(): Promise<void> {
  const g = stateFor(SECOND)
  if (!g) return
  try {
    await workspace.call('close_slot', { slot: SECOND })
  } catch (e) {
    toast(errorMessage(e), 'error')
    return
  }
  if (active() === g) {
    window.location.hash = '#/journals'
    await activate('')
  }
  g.dispose()
  graphs.list = graphs.list.filter((x) => x !== g)
  try {
    localStorage.removeItem(REMEMBERED)
  } catch {
    /* nothing to forget */
  }
}

/** Opens the graph in `path` in `slot` (default: the active graph's) and shows it. */
export async function openGraph(path: string, create: boolean, slot = backend.slot) {
  try {
    await adoptGraph(await backendFor(slot).call('open_graph', { path, create }), slot)
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
}

/** Makes `info` (a graph the backend has just opened) the graph shown. Used after `open_graph` and after
 * `create_remote_graph`, which opens the new graph itself. */
export async function adoptGraph(info: GraphInfo, slot = backend.slot) {
  if (app.graph) {
    // another graph: nothing of the pages loaded so far applies, so start the page over. Until the reload
    // finishes the screen stays as it is, so nothing of the new graph is shown with the old pages.
    window.location.hash = routeHash({ kind: 'journals' }, slot)
    window.location.reload()
    return
  }
  mount(slot, info)
  await activate(slot)
  app.needsGraph = false
  app.today = (await backend.call('today', {})).value
  await refreshConflicts()
  await loadDeviceSettings()
}
