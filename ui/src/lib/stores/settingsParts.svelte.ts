// The part of a long Settings section that is open (Sync: server or encryption; Calendar: account, calendars or
// tasks), and a way for other places to open Settings on a given part. Kept in the browser like the open section.
export type Parts = {
  sync: 'server' | 'encryption'
  calendar: 'account' | 'calendars' | 'tasks'
}

const KEY = 'notatnik-settings-parts'
const DEFAULTS: Parts = { sync: 'server', calendar: 'account' }

function load(): Parts {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<Parts>
    return {
      sync: saved.sync === 'encryption' ? 'encryption' : DEFAULTS.sync,
      calendar:
        saved.calendar === 'calendars' || saved.calendar === 'tasks' ? saved.calendar : DEFAULTS.calendar,
    }
  } catch {
    return { ...DEFAULTS }
  }
}

/** Which part of each long section is open. A pane binds its sub-navigation to one field. */
export const parts = $state<Parts>(load())

/** Writes the open parts to the browser; a pane calls it from an `$effect` that reads them. */
export function saveParts() {
  try {
    localStorage.setItem(KEY, JSON.stringify($state.snapshot(parts)))
  } catch {
    /* storage may be blocked: the parts are then forgotten on reload */
  }
}

/** Set by `goToSettings`; Settings switches to that section when it sees it, and clears it. */
export const settingsGo = $state({ section: null as 'sync' | 'calendar' | null })

/** Opens Settings on a part of a section: the link that calls it still goes to `#/settings` by itself. */
export function goToSettings<S extends keyof Parts>(section: S, part: Parts[S]) {
  parts[section] = part
  try {
    localStorage.setItem('notatnik-settings-tab', section)
  } catch {
    /* see saveParts */
  }
  settingsGo.section = section
}
