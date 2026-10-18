<script lang="ts">
  // The "Calendar" part of a journal day (JR-5): the day's events from the calendars chosen in Settings → Calendar.
  // Read only: events are not part of the notes. Nothing is drawn when the day has none.
  import type { CalendarEventView } from '../api'
  import { eventsOn, eventTime, loadEvents } from '../events'
  import { calendarAuto } from '../stores/calendarAuto.svelte'

  let { date }: { date: string } = $props()
  let events = $state<CalendarEventView[]>([])

  // `calendarAuto.last` changes after every sync, so a new sync refreshes the list
  $effect(() => {
    void calendarAuto.last
    const day = date
    let stale = false
    void loadEvents(day, day).then((e) => {
      if (!stale) events = e
    })
    return () => (stale = true)
  })
  const today = $derived(eventsOn(events, date))
</script>

{#if today.length}
  <section class="events" aria-label="Calendar" data-testid="day-events">
    <h4>Calendar</h4>
    <ul>
      {#each today as e (e.uid + e.date + e.start)}
        <li>
          <span class="time">{eventTime(e, date)}</span>
          <span class="title">{e.title}</span>
          {#if e.location}<span class="where">{e.location}</span>{/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .events {
    margin: 0.4em 0 0.8em;
    padding-left: 0.8em;
    border-left: 3px solid var(--border);
  }
  h4 {
    margin: 0 0 0.2em;
    font-size: 0.8em;
    color: var(--muted);
    font-weight: 600;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  li {
    display: flex;
    gap: 0.6em;
    padding: 0.1em 0;
  }
  .time {
    min-width: 6.5em;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .where {
    color: var(--muted);
  }
</style>
