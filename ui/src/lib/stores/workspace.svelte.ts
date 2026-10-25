// The graphs of this device and the ones that are open in a pane (MG-1). The dialogs that take notes to another graph
// and the graph switcher read it; `refreshWorkspace` loads it again.
import { backend, type GraphHistoryEntry, type SlotInfo } from '../api'

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
