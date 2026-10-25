// Which graph a component belongs to. A sidebar section provides its graph to everything inside it; a component of the
// page area gets the active graph. Components read `useGraph()` once, when they are made: the page area is made anew when
// another graph becomes active, and a section's graph never changes.
import { getContext, setContext } from 'svelte'
import { active } from './stores/active.svelte'
import type { GraphState } from './stores/graphState.svelte'

const KEY = Symbol('graph')

/** Called by a section: the graph of everything below it. */
export function provideGraph(g: GraphState): void {
  setContext(KEY, g)
}

/** The graph of this component: its section's, or the active graph. */
export function useGraph(): GraphState {
  return getContext<GraphState | undefined>(KEY) ?? active()
}
