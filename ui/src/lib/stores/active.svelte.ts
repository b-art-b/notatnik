// The active graph: the one the page area shows. The modules the page area imports (`recent`, `favorites`, `icons`,
// `archived`, `encryption`, `sync`, `calendarAuto`, `calendarTargets`, `bus`) are thin views of `active().…`, so the
// code that was written for one graph keeps working while the sidebar holds two.
import { GraphState } from './graphState.svelte'

const holder = $state<{ g: GraphState | null }>({ g: null })
/** What answers before any graph is open (the welcome screen): empty lists, nothing loaded. */
let none: GraphState | undefined

/** The state of the active graph (empty before a graph is open). */
export function active(): GraphState {
  return holder.g ?? (none ??= new GraphState('', null))
}

export function setActive(g: GraphState | null) {
  holder.g = g
}
