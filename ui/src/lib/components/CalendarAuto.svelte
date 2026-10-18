<script lang="ts">
  // Starts the calendar sync of the open graph and keeps it going (see stores/calendarAuto.svelte.ts). It draws
  // nothing. The two effects are Svelte 5 "runes": each re-runs when the state it reads changes.
  import { untrack } from 'svelte'
  import { app } from '../stores/app.svelte'
  import { bus } from '../stores/events.svelte'
  import { refreshCalendarTargets } from '../stores/calendarTargets.svelte'
  import { AFTER_CHANGE_MS, scheduleCalendarSync, stopCalendarSync } from '../stores/calendarAuto.svelte'

  // a graph was opened (or switched): sync soon; closed: stop
  $effect(() => {
    if (app.graph?.id) {
      untrack(() => {
        scheduleCalendarSync(1500)
        void refreshCalendarTargets()
      })
      return () => stopCalendarSync()
    }
  })

  // the notes changed on disk or in the app: sync a few seconds after the last change
  let seen = 0
  $effect(() => {
    const v = bus.version
    untrack(() => {
      if (v !== seen && app.graph?.id) scheduleCalendarSync(AFTER_CHANGE_MS)
      seen = v
    })
  })
</script>
