// Android home-screen widgets (AN-5..AN-7, docs/phases/android-widgets.md): the app side. When the app starts, comes back
// to the foreground or goes away, and a little after the notes changed, it asks the backend to add what the widget's
// dialog captured and to write the snapshot of today's tasks; a `notatnik://` link the widget opened the app with
// becomes a route. On a computer none of this runs.
import { backendFor } from './api'
import { isMobile } from './platform'
import type { Route } from './route'
import { app, navigate } from './stores/app.svelte'
import { sync, syncNow } from './stores/sync.svelte'

/** The route a widget link stands for, or `null` for anything else. A link is only a request: a page name is looked up
 * like a typed one, so an unknown name just shows the usual "no such page". */
export function linkRoute(link: string): Route | null {
  const m = /^notatnik:\/\/(capture|agenda|page)(?:\/(.*))?$/.exec(link.trim())
  if (!m) return null
  if (m[1] !== 'page') return m[2] ? null : { kind: m[1] as 'capture' | 'agenda' }
  let name: string
  try {
    name = decodeURIComponent(m[2] ?? '')
  } catch {
    return null
  }
  // eslint-disable-next-line no-control-regex
  if (!name || name.length > 300 || /[\u0000-\u001f]/.test(name)) return null
  return { kind: 'page', name, zoom: [] }
}

let timer: ReturnType<typeof setTimeout> | undefined
let started = false

/** Adds the captured lines, writes the snapshot, and (when lines were added) starts a sync so they leave the phone. */
export async function syncWidgets(): Promise<void> {
  if (!isMobile || !app.graph) return
  try {
    const r = await backendFor('').call('widgets_sync', {})
    if (r.drained > 0 && !sync.busy) await syncNow(false)
  } catch {
    /* no graph yet, or the files are not writable: the next change tries again */
  }
}

/** The notes changed: the snapshot is written a moment later (not once per keystroke). */
export function scheduleWidgetSync(): void {
  if (!isMobile) return
  clearTimeout(timer)
  timer = setTimeout(() => void syncWidgets(), 3000)
}

async function openLink(): Promise<void> {
  try {
    const r = await backendFor('').call('take_link', {})
    const route = r.value ? linkRoute(r.value) : null
    if (route) navigate(route)
  } catch {
    /* no link */
  }
}

/** Starts it (once) when the graph is open: now, and whenever the app is shown or hidden. */
export function startWidgets(): void {
  if (!isMobile || started) return
  started = true
  void openLink().then(syncWidgets)
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') void openLink().then(syncWidgets)
    else void syncWidgets() // going away: the widget shows what the app knows now
  })
}
