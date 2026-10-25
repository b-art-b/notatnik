// The graphs of this device and the ones that are open in a pane (MG-1). The dialogs that take notes to another graph
// and the graph switcher read it; `refreshWorkspace` loads it again.
import { backend, backendFor, CommandError, type GraphHistoryEntry, type SlotInfo } from '../api'
import { errorMessage, toast } from './app.svelte'

export const known = $state({
  /** The graphs open in a pane, the first graph first. */
  slots: [] as SlotInfo[],
  /** Every graph this device has opened, most recent first. */
  graphs: [] as GraphHistoryEntry[],
})

export async function refreshWorkspace(): Promise<void> {
  try {
    const [slots, graphs] = await Promise.all([backend.call('slots', {}), backend.call('list_graphs', {})])
    known.slots = slots
    known.graphs = graphs
  } catch {
    /* no graph yet: the lists stay as they were */
  }
}

/** Is there a graph to take notes to? */
export function hasOtherGraph(ownId: string | undefined): boolean {
  return known.graphs.some((g) => g.id !== ownId && g.exists)
}

/** Bumped when the graph in the pane changes, so the frame starts over (its stores belong to the graph it showed). */
export const pane = $state({ version: 0 })

/** Opens the graph in `path` in the pane beside the first graph (MG-6), or shows another graph there. */
export async function openBeside(path: string): Promise<void> {
  try {
    await backendFor(BESIDE).call('open_graph', { path, create: false })
  } catch (e) {
    toast(
      e instanceof CommandError && e.code === 'already_open'
        ? 'That graph is open already.'
        : errorMessage(e),
      'error',
    )
    return
  }
  pane.version += 1
  await refreshWorkspace()
}

/** Closes the pane (the graph stays on disk and in the list of graphs). */
export async function closeBeside(): Promise<void> {
  try {
    await backend.call('close_slot', { slot: BESIDE })
  } catch (e) {
    toast(errorMessage(e), 'error')
  }
  await refreshWorkspace()
}

/** The slot the pane beside the first graph uses. */
export const BESIDE = '1'

/** The graph in the pane, if one is open. */
export const beside = () => known.slots.find((s) => s.slot === BESIDE)
