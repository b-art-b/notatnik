// Dates shown in the UI. `formatDate` reads module-level settings that Svelte can't see, so a heading
// rendered before the device settings finished loading would keep the old format. `shownDate` reads
// `app.dateVersion` (bumped whenever the date settings change), which makes the caller re-run.
import { formatDate } from '../dates'
import { app } from './app.svelte'

export function shownDate(iso: string): string {
  void app.dateVersion
  return formatDate(iso)
}
