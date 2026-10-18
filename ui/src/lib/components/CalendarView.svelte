<script lang="ts">
  // The calendar view of the Agenda (CV-1..CV-8): tasks and the events of your calendars on a day, week or month,
  // with hours, lengths, colours per calendar, dragging, resizing and a tray for tasks without a date.
  //
  // For readers new to Svelte 5: `$state` is a value the screen follows; `$derived` is a value computed from
  // others and kept up to date; `$effect` runs after the screen changed and may return a clean-up function. The
  // maths (which minute a block starts at, how overlapping blocks share the width, which text a drag writes) is in
  // `calendarGrid.ts`, where it is tested without a browser.
  import { untrack } from 'svelte'
  import { backend, type CalendarEventView, type ResultRow } from '../api'
  import { app, errorMessage, toast } from '../stores/app.svelte'
  import { bus } from '../stores/events.svelte'
  import { calendarAuto } from '../stores/calendarAuto.svelte'
  import { calendarOf, calendarTargets, setCalendarTargets } from '../stores/calendarTargets.svelte'
  import { getDoc } from '../stores/pages.svelte'
  import { jumpToBlock } from '../jump'
  import { loadEvents } from '../events'
  import { setProperty, taskStateOp } from '../commands/exec'
  import { addMonths, monthGrid, moveDateValue, weekOf } from '../views'
  import {
    DAY_MINUTES,
    SNAP_MINUTES,
    addDays,
    colorFor,
    eventSegments,
    layoutDay,
    movedValue,
    newTaskText,
    placingField,
    rolledOver,
    monthTitle,
    rangeLabel,
    resizedValue,
    snap,
    taskSegment,
    toClock,
    type DateField,
    type DateMode,
    type Segment,
    weekdayShort,
  } from '../calendarGrid'

  /** The agenda's filters as a query (namespace, tag, priority, show done). */
  let { queryText }: { queryText: string } = $props()

  const HOUR = 48 // pixels per hour
  const PX = HOUR / 60

  type Mode = 'day' | 'week' | 'month'
  let mode = $state<Mode>('week')
  let dateMode = $state<DateMode>('earlier')
  /** Overdue open tasks at the day they were planned for, or all on today (only a way of looking at them). */
  let overdue = $state<'dates' | 'today'>('dates')
  let showTasks = $state(true)
  let showEvents = $state(true)
  let hidden = $state<string[]>([]) // calendars switched off in the legend, by address
  try {
    const saved = JSON.parse(localStorage.getItem('notatnik-calendar-view') ?? '{}')
    if (['day', 'week', 'month'].includes(saved.mode)) mode = saved.mode
    if (['earlier', 'scheduled', 'deadline'].includes(saved.dateMode)) dateMode = saved.dateMode
    if (saved.overdue === 'today') overdue = 'today'
    showTasks = saved.showTasks !== false
    showEvents = saved.showEvents !== false
    if (Array.isArray(saved.hidden)) hidden = saved.hidden.filter((h: unknown) => typeof h === 'string')
  } catch {
    /* defaults */
  }
  $effect(() => {
    try {
      localStorage.setItem(
        'notatnik-calendar-view',
        JSON.stringify({ mode, dateMode, overdue, showTasks, showEvents, hidden }),
      )
    } catch {
      /* not remembered */
    }
  })

  const weekStart = $derived(app.graph?.week_start === 'sunday' ? 'sunday' : 'monday')
  const today = $derived(app.today || new Date().toISOString().slice(0, 10))
  // `anchor` is a day inside what is shown; the week or the month around it is drawn. It is today until you move:
  // `app.today` can still be empty when this view is built, so it is not copied at that moment.
  let moved = $state<string | null>(null)
  const anchor = $derived(moved ?? today)
  const weeks = $derived(mode === 'month' ? monthGrid(anchor.slice(0, 7), weekStart) : [])
  const days = $derived(
    mode === 'day' ? [anchor] : mode === 'week' ? weekOf(anchor, weekStart).map((d) => d.date) : [],
  )
  const visible = $derived(mode === 'month' ? weeks.flat().map((d) => d.date) : days)
  const first = $derived(visible[0])
  const last = $derived(visible[visible.length - 1])

  function step(n: number) {
    if (mode === 'month') moved = `${addMonths(anchor.slice(0, 7), n)}-01`
    else moved = addDays(anchor, n * (mode === 'week' ? 7 : 1))
  }
  const heading = $derived(mode === 'month' ? monthTitle(anchor) : rangeLabel(days))
  const weekday = weekdayShort

  // --- data: the tasks (a query, like the other agenda views) and the events of the chosen calendars ---
  let rows = $state<ResultRow[]>([])
  $effect(() => {
    void bus.version
    const text = queryText
    let cancelled = false
    const timer = setTimeout(async () => {
      try {
        const r = await backend.call('run_query', { text, view: 'timeline', page: null })
        if (!cancelled) rows = r.rows
      } catch {
        /* an invalid filter keeps the previous result */
      }
    }, 100)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  })

  let events = $state<CalendarEventView[]>([])
  $effect(() => {
    void calendarAuto.last
    void bus.version
    const from = first
    const to = last
    let stale = false
    void loadEvents(from, to).then((e) => {
      if (!stale) events = e
    })
    return () => (stale = true)
  })

  /** The title of a task block as one plain line: no marker, priority or properties. */
  function titleOf(r: ResultRow): string {
    const line = (r.block?.text ?? '').split('\n')[0]
    return line
      .replace(/^(TODO|DOING|DONE|LATER|NOW|WAITING|CANCELED|CANCELLED)\s+/, '')
      .replace(/^\[#\w+\]\s*/, '')
      .trim()
  }
  const isDone = (r: ResultRow) => r.task_state === 'done' || r.task_state === 'canceled'
  const calProp = (r: ResultRow) => r.block?.props.find((p) => p.key.toLowerCase() === 'cal')?.raw.trim()
  /** The calendar a task is synced to, when calendar sync is on (`undefined`: not known, `null`: `cal:: no`). */
  const calendarOfTask = (r: ResultRow) => (calendarTargets.on ? calendarOf(calProp(r)) : undefined)

  interface Placed {
    key: string
    seg: Segment
    color: string
    /** The calendar's address; empty for a task that is not synced. */
    calendar: string
    row?: ResultRow
    field?: DateField
    value?: string
    event?: CalendarEventView
    done: boolean
    /** Shown on today though planned for an earlier day (overdue tasks rolled over). */
    rolled?: { from: string; days: number }
  }

  const placed = $derived.by(() => {
    const out: Placed[] = []
    if (showTasks) {
      for (const r of rows) {
        if (!r.block?.task) continue
        const pf = placingField(r.block?.task?.scheduled ?? null, r.block?.task?.deadline ?? null, dateMode)
        if (!pf) continue
        let seg = taskSegment(r.id, titleOf(r), pf.value, pf.field)
        // overdue and open: all on today, when today is in view (a past week still shows what happened when)
        let rolled: Placed['rolled']
        if (overdue === 'today' && !isDone(r) && first <= today && today <= last) {
          const ro = rolledOver(seg, today)
          if (ro) {
            seg = ro.seg
            rolled = { from: ro.from, days: ro.days }
          }
        }
        if (seg.date < first || seg.date > last) continue
        const cal = calendarOfTask(r)
        if (cal && hidden.includes(cal.href)) continue
        out.push({
          key: r.id,
          seg,
          color: cal ? colorFor(cal.href, calendarTargets.colors) : 'var(--task-todo)',
          calendar: cal?.href ?? '',
          row: r,
          field: pf.field,
          value: pf.value,
          done: isDone(r),
          rolled,
        })
      }
    }
    if (showEvents) {
      for (const e of events) {
        if (hidden.includes(e.calendar)) continue
        for (const seg of eventSegments(e, first, last)) {
          out.push({
            key: seg.id,
            seg,
            color: colorFor(e.calendar, calendarTargets.colors),
            calendar: e.calendar,
            event: e,
            done: false,
          })
        }
      }
    }
    return out
  })

  /** Tasks with no date for the chosen field: the tray. */
  const unscheduled = $derived(
    showTasks
      ? rows.filter(
          (r) =>
            r.block?.task &&
            !isDone(r) &&
            !placingField(r.block?.task?.scheduled ?? null, r.block?.task?.deadline ?? null, dateMode),
        )
      : [],
  )

  const allDayOn = (date: string) => placed.filter((p) => p.seg.date === date && p.seg.allDay)
  const timedOn = (date: string) =>
    placed.filter((p) => p.seg.date === date && !p.seg.allDay).sort((a, b) => a.seg.start - b.seg.start)
  const layouts = $derived.by(() => {
    const m = new Map<string, ReturnType<typeof layoutDay>>()
    for (const d of days) m.set(d, layoutDay(timedOn(d).map((p) => ({ ...p.seg, id: p.key }))))
    return m
  })
  const slot = (date: string, key: string) => layouts.get(date)?.get(key) ?? { col: 0, cols: 1 }

  // --- the legend: every calendar in use, its colour, and a switch ---
  const legend = $derived(
    Object.entries(calendarTargets.names)
      .map(([href, name]) => ({ href, name, color: colorFor(href, calendarTargets.colors) }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  )
  function toggleCalendar(href: string, on: boolean) {
    hidden = on ? hidden.filter((h) => h !== href) : [...hidden.filter((h) => h !== href), href]
  }
  async function chooseColor(href: string, color: string) {
    try {
      setCalendarTargets(await backend.call('calendar_set_color', { href, color }))
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  // --- the clock: a line at the current time on today's column ---
  const clockMinutes = () => {
    const n = new Date()
    return n.getHours() * 60 + n.getMinutes()
  }
  let nowMin = $state(clockMinutes())
  $effect(() => {
    const t = setInterval(() => (nowMin = clockMinutes()), 60_000)
    return () => clearInterval(t)
  })
  let scroller = $state<HTMLDivElement>()
  // when the hours appear, scroll to a little before now (today shown) or to 07:00
  $effect(() => {
    const box = scroller
    if (!box) return
    untrack(() => {
      const focus = days.includes(today) ? nowMin : 7 * 60
      box.scrollTop = Math.max(0, focus - 60) * PX
    })
  })

  // --- dragging with the pointer (as in the kanban and the timeline: works in every webview) ---
  const THRESHOLD = 5
  type Press = {
    kind: 'move' | 'resize' | 'tray'
    p?: Placed
    row?: ResultRow
    x: number
    y: number
    /** Minutes between the top of the block and where it was grabbed. */
    grab: number
  }
  let press: Press | null = null
  type Drop =
    | { where: 'grid'; date: string; start: number; end: number }
    | { where: 'allday'; date: string }
    | { where: 'cell'; date: string }
    | null
  let drag = $state<{ kind: Press['kind']; key: string; drop: Drop } | null>(null)
  let justDragged = false

  function dropAt(x: number, y: number, p: Press): Drop {
    const el = document.elementFromPoint(x, y)
    const col = el?.closest<HTMLElement>('[data-col]')
    if (col) {
      const rect = col.getBoundingClientRect()
      const date = col.dataset.col as string
      if (p.kind === 'resize') {
        const end = snap((y - rect.top) / PX)
        return { where: 'grid', date, start: p.p?.seg.start ?? 0, end }
      }
      const start = Math.max(0, Math.min(DAY_MINUTES - SNAP_MINUTES, snap((y - rect.top) / PX - p.grab)))
      const len = p.p ? p.p.seg.end - p.p.seg.start : 60
      return { where: 'grid', date, start, end: Math.min(DAY_MINUTES, start + len) }
    }
    if (p.kind === 'resize') return null
    const strip = el?.closest<HTMLElement>('[data-allday]')
    if (strip) return { where: 'allday', date: strip.dataset.allday as string }
    const cell = el?.closest<HTMLElement>('[data-cell]')
    if (cell) return { where: 'cell', date: cell.dataset.cell as string }
    return null
  }

  function down(e: PointerEvent, kind: Press['kind'], p?: Placed, row?: ResultRow) {
    if (e.button !== 0) return
    if (kind !== 'tray' && !p?.row) return // events are read only
    const top = (e.currentTarget as HTMLElement).getBoundingClientRect().top
    press = {
      kind,
      p,
      row: row ?? p?.row,
      x: e.clientX,
      y: e.clientY,
      // a task from the tray has no block yet: its top is where the pointer is
      grab: kind === 'tray' ? 0 : Math.max(0, (e.clientY - top) / PX),
    }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    if (kind === 'resize') e.stopPropagation()
  }
  function move(e: PointerEvent) {
    if (!press) return
    if (!drag && Math.hypot(e.clientX - press.x, e.clientY - press.y) < THRESHOLD) return
    drag = {
      kind: press.kind,
      key: press.p?.key ?? press.row?.id ?? '',
      drop: dropAt(e.clientX, e.clientY, press),
    }
  }
  async function up(e: PointerEvent) {
    const pr = press
    const d = drag
    press = null
    drag = null
    if (!pr) return
    if (!d) return // a click: the element's own click handler
    justDragged = true
    setTimeout(() => (justDragged = false), 0)
    const drop = dropAt(e.clientX, e.clientY, pr)
    const row = pr.row
    if (!drop || !row?.block) return
    const field: DateField = pr.p?.field ?? 'scheduled'
    const current = pr.p?.value ?? ''
    let next: string | null = null
    if (pr.kind === 'resize') {
      if (drop.where === 'grid') next = resizedValue(current, drop.end)
    } else if (drop.where === 'grid') {
      next = movedValue(current, field, drop.date, drop.start)
    } else if (drop.where === 'allday') {
      next = drop.date
    } else {
      next = current ? moveDateValue(current, drop.date) : drop.date
    }
    if (next && next !== current) await write(row, field, next)
  }
  async function write(row: ResultRow, field: DateField, value: string) {
    if (!row.block) return
    try {
      await setProperty(getDoc(row.page), row.block.path, field, value)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  // --- clicks and keys ---
  let picked = $state<CalendarEventView | null>(null)
  function open(p: Placed) {
    if (justDragged) return
    if (p.row?.block) jumpToBlock(p.row.page, p.row.block.path)
    else if (p.event) picked = p.event
  }
  function key(e: KeyboardEvent, p: Placed) {
    if (e.key === 'Enter' && e.target === e.currentTarget) return open(p)
    if (!e.altKey || !p.row?.block || !p.field || !p.value) return
    const arrows = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown']
    if (!arrows.includes(e.key)) return
    e.preventDefault()
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      void write(p.row, p.field, moveDateValue(p.value, addDays(p.seg.date, e.key === 'ArrowLeft' ? -1 : 1)))
    } else if (!p.seg.allDay) {
      const delta = e.key === 'ArrowUp' ? -SNAP_MINUTES : SNAP_MINUTES
      const start = Math.max(0, Math.min(DAY_MINUTES - SNAP_MINUTES, p.seg.start + delta))
      void write(p.row, p.field, movedValue(p.value, p.field, p.seg.date, start))
    }
  }
  async function toggleDone(p: Placed) {
    const r = p.row
    if (!r?.block) return
    try {
      await getDoc(r.page).apply([taskStateOp(r.block.path, p.done ? 'TODO' : 'DONE')])
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  // --- making a task by clicking an empty slot: it goes into today's journal ---
  let creating = $state<{ date: string; start: number } | null>(null)
  let newTitle = $state('')
  let titleInput = $state<HTMLInputElement>()
  $effect(() => {
    if (creating) titleInput?.focus()
  })
  function startCreate(e: MouseEvent, date: string) {
    if (e.target !== e.currentTarget || justDragged) return
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const start = Math.max(
      0,
      Math.min(
        DAY_MINUTES - SNAP_MINUTES,
        Math.floor((e.clientY - rect.top) / PX / SNAP_MINUTES) * SNAP_MINUTES,
      ),
    )
    newTitle = ''
    creating = { date, start }
  }
  async function create() {
    const c = creating
    const title = newTitle.trim()
    creating = null
    newTitle = ''
    if (!c || !title) return
    const text = newTaskText(title, c.date, c.start)
    try {
      await getDoc(app.today).applyWith((view) => [
        { op: 'insert', parent: [], index: view?.blocks.length ?? 0, text },
      ])
      toast("Added to today's journal")
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  const timeText = (seg: Segment) =>
    seg.allDay ? '' : `${toClock(seg.start)}${seg.kind === 'event' || !seg.due ? `–${toClock(seg.end)}` : ''}`
  const monthChips = (date: string) =>
    placed
      .filter((p) => p.seg.date === date)
      .sort((a, b) => Number(!a.seg.allDay) - Number(!b.seg.allDay) || a.seg.start - b.seg.start)
  const ghost = $derived(drag?.drop?.where === 'grid' ? drag.drop : null)
</script>

{#snippet item(p: Placed)}
  <!-- a block of the grid: a task (outlined, with a checkbox) or an event (filled, read only) -->
  <div
    class="item"
    class:task={!!p.row}
    class:done={p.done}
    class:dragging={drag?.key === p.key && drag.kind !== 'tray'}
    class:due={p.seg.due}
    class:rolled={!!p.rolled}
    data-testid="cal-item"
    data-kind={p.seg.kind}
    data-date={p.seg.date}
    data-start={p.seg.allDay ? '' : p.seg.start}
    data-end={p.seg.allDay ? '' : p.seg.end}
    style:--c={p.color}
    role="button"
    tabindex="0"
    aria-label={`${timeText(p.seg)} ${p.seg.title}`.trim()}
    onpointerdown={(e) => down(e, 'move', p)}
    onpointermove={move}
    onpointerup={up}
    onclick={() => open(p)}
    onkeydown={(e) => key(e, p)}
  >
    {#if p.row}<input
        type="checkbox"
        class="check"
        aria-label={`Done: ${p.seg.title}`}
        checked={p.done}
        onpointerdown={(e) => e.stopPropagation()}
        onclick={(e) => e.stopPropagation()}
        onchange={() => toggleDone(p)}
      />{/if}
    {#if p.rolled}<span class="roll" title={`Planned for ${p.rolled.from}`} data-testid="cal-rolled"
        >↻ {p.rolled.days}d</span
      >{/if}
    <span class="at">{timeText(p.seg)}</span>
    <span class="t">{p.seg.title}</span>
    {#if p.event?.location && !p.seg.allDay}<span class="loc">{p.event.location}</span>{/if}
    {#if p.row && p.field === 'scheduled' && !p.seg.allDay}
      <span
        class="grip"
        role="separator"
        aria-label="Change the end time"
        onpointerdown={(e) => down(e, 'resize', p)}
        onpointermove={move}
        onpointerup={up}
      ></span>
    {/if}
  </div>
{/snippet}

<div class="bar" role="toolbar" aria-label="Calendar">
  <div class="nav">
    <button type="button" aria-label="Previous" onclick={() => step(-1)}>‹</button>
    <button type="button" data-testid="cal-today" onclick={() => (moved = null)}>Today</button>
    <button type="button" aria-label="Next" onclick={() => step(1)}>›</button>
  </div>
  <strong class="heading" data-testid="cal-heading">{heading}</strong>
  <div class="modes" role="group" aria-label="Calendar range">
    {#each ['day', 'week', 'month'] as m (m)}
      <button
        type="button"
        class="mode"
        class:on={mode === m}
        aria-pressed={mode === m}
        onclick={() => (mode = m as Mode)}>{m}</button
      >
    {/each}
  </div>
</div>
<div class="legend" aria-label="Calendars">
  <label><input type="checkbox" bind:checked={showTasks} /> Tasks</label>
  <label><input type="checkbox" bind:checked={showEvents} /> Events</label>
  <label
    >Placed by
    <select bind:value={dateMode} aria-label="Placed by">
      <option value="earlier">earlier date</option>
      <option value="scheduled">scheduled</option>
      <option value="deadline">deadline</option>
    </select>
  </label>
  <label
    >Overdue tasks
    <select bind:value={overdue} aria-label="Overdue tasks">
      <option value="dates">at their dates</option>
      <option value="today">rolled over to today</option>
    </select>
  </label>
  {#each legend as c (c.href)}
    <span class="cal-key" data-testid="cal-legend-item">
      <input
        type="checkbox"
        aria-label={`Show ${c.name}`}
        checked={!hidden.includes(c.href)}
        onchange={(e) => toggleCalendar(c.href, e.currentTarget.checked)}
      />
      <input
        type="color"
        class="swatch"
        aria-label={`Colour of ${c.name}`}
        value={c.color}
        onchange={(e) => chooseColor(c.href, e.currentTarget.value)}
      />
      {c.name}
    </span>
  {/each}
</div>

<div class="layout">
  <div class="main">
    {#if mode !== 'month'}
      <div class="cal" data-testid="cal-grid" style:--n={days.length}>
        <div class="row head">
          <div class="gutter"></div>
          {#each days as d (d)}
            <button
              type="button"
              class="dayhead"
              class:today={d === today}
              onclick={() => {
                moved = d
                mode = 'day'
              }}><span class="wd">{weekday(d)}</span> <span class="dn">{Number(d.slice(8))}</span></button
            >
          {/each}
        </div>
        <div class="row strip">
          <div class="gutter">all day</div>
          {#each days as d (d)}
            <div
              class="allday"
              class:over={drag?.drop?.where === 'allday' && drag.drop.date === d}
              data-allday={d}
              data-testid="cal-allday"
            >
              {#each allDayOn(d) as p (p.key)}{@render item(p)}{/each}
            </div>
          {/each}
        </div>
        <div class="scroll" bind:this={scroller} data-testid="cal-scroll">
          <div class="row body" style:height="{DAY_MINUTES * PX}px">
            <div class="gutter hours">
              {#each Array.from({ length: 24 }, (_, i) => i) as h (h)}
                <div class="hour" style:height="{HOUR}px">
                  {h === 0 ? '' : `${String(h).padStart(2, '0')}:00`}
                </div>
              {/each}
            </div>
            {#each days as d (d)}
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
              <div
                class="col"
                class:today={d === today}
                data-col={d}
                data-testid="cal-col"
                onclick={(e) => startCreate(e, d)}
              >
                {#each timedOn(d) as p (p.key)}
                  {@const s = slot(d, p.key)}
                  <div
                    class="pos"
                    style:top="{p.seg.start * PX}px"
                    style:height="{Math.max(18, (p.seg.end - p.seg.start) * PX - 1)}px"
                    style:left="calc({s.col} / {s.cols} * 100%)"
                    style:width="calc(100% / {s.cols} - 2px)"
                  >
                    {@render item(p)}
                  </div>
                {/each}
                {#if ghost && ghost.date === d}
                  <div
                    class="ghost"
                    data-testid="cal-ghost"
                    style:top="{ghost.start * PX}px"
                    style:height="{Math.max(18, (ghost.end - ghost.start) * PX - 1)}px"
                  >
                    {toClock(ghost.start)}–{toClock(ghost.end)}
                  </div>
                {/if}
                {#if creating && creating.date === d}
                  <form
                    class="create"
                    style:top="{creating.start * PX}px"
                    onsubmit={(e) => {
                      e.preventDefault()
                      void create()
                    }}
                  >
                    <input
                      bind:this={titleInput}
                      bind:value={newTitle}
                      aria-label="New task"
                      placeholder="{toClock(creating.start)} New task…"
                      onkeydown={(e) => {
                        e.stopPropagation()
                        if (e.key === 'Escape') creating = null
                      }}
                      onblur={() => (creating = null)}
                    />
                  </form>
                {/if}
                {#if d === today}<div
                    class="now"
                    style:top="{nowMin * PX}px"
                    data-testid="cal-now"
                  ></div>{/if}
              </div>
            {/each}
          </div>
        </div>
      </div>
    {:else}
      <table class="month" data-testid="cal-month">
        <thead>
          <tr>
            {#each weeks[0] as d (d.date)}<th>{weekday(d.date)}</th>{/each}
          </tr>
        </thead>
        <tbody>
          {#each weeks as w, i (i)}
            <tr>
              {#each w as d (d.date)}
                {@const chips = monthChips(d.date)}
                <td
                  class:out={!d.inMonth}
                  class:today={d.date === today}
                  class:over={drag?.drop?.where === 'cell' && drag.drop.date === d.date}
                  data-cell={d.date}
                >
                  <button
                    type="button"
                    class="num"
                    onclick={() => {
                      moved = d.date
                      mode = 'day'
                    }}>{Number(d.date.slice(8))}</button
                  >
                  {#each chips.slice(0, 4) as p (p.key)}
                    <div
                      class="chip"
                      class:task={!!p.row}
                      class:done={p.done}
                      class:dragging={drag?.key === p.key && drag.kind !== 'tray'}
                      data-testid="cal-item"
                      data-kind={p.seg.kind}
                      data-date={p.seg.date}
                      style:--c={p.color}
                      role="button"
                      tabindex="0"
                      onpointerdown={(e) => down(e, 'move', p)}
                      onpointermove={move}
                      onpointerup={up}
                      onclick={() => open(p)}
                      onkeydown={(e) => key(e, p)}
                    >
                      {#if p.rolled}<span class="roll" title={`Planned for ${p.rolled.from}`}
                          >↻ {p.rolled.days}d</span
                        >{/if}
                      {#if !p.seg.allDay}<span class="at">{toClock(p.seg.start)}</span>{/if}
                      {p.seg.title}
                    </div>
                  {/each}
                  {#if chips.length > 4}
                    <button
                      type="button"
                      class="more"
                      onclick={() => {
                        moved = d.date
                        mode = 'day'
                      }}>+{chips.length - 4} more</button
                    >
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <aside class="tray" aria-label="Tasks without a date" data-testid="cal-tray">
    <h4>No date <span class="n">{unscheduled.length}</span></h4>
    <p class="muted">Drag a task onto the calendar to give it a date and time.</p>
    {#each unscheduled as r (r.id)}
      <div
        class="tray-item"
        class:dragging={drag?.key === r.id && drag.kind === 'tray'}
        data-testid="cal-tray-item"
        role="button"
        tabindex="0"
        onpointerdown={(e) => down(e, 'tray', undefined, r)}
        onpointermove={move}
        onpointerup={up}
        onclick={() => !justDragged && r.block && jumpToBlock(r.page, r.block.path)}
        onkeydown={(e) => e.key === 'Enter' && r.block && jumpToBlock(r.page, r.block.path)}
      >
        {titleOf(r)}
      </div>
    {/each}
  </aside>
</div>

{#if picked}
  {@const e = picked}
  <div class="popup-back" role="presentation" onclick={() => (picked = null)}></div>
  <div class="popup" role="dialog" aria-label="Event" data-testid="cal-event-popup">
    <strong>{e.title}</strong>
    <div>
      {e.date}{e.end_date !== e.date ? ` – ${e.end_date}` : ''}
      {e.start ? `${e.start}${e.end ? `–${e.end}` : ''}` : 'all day'}
    </div>
    <div class="muted">{calendarTargets.names[e.calendar] ?? e.calendar}</div>
    {#if e.location}<div>{e.location}</div>{/if}
    <button type="button" onclick={() => (picked = null)}>Close</button>
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 0.8em;
    flex-wrap: wrap;
    margin: 0.4em 0;
  }
  .nav {
    display: flex;
    gap: 0.25em;
  }
  .heading {
    min-width: 11em;
  }
  .modes {
    margin-left: auto;
    display: flex;
    gap: 0.25em;
  }
  .mode.on {
    background: var(--selection);
    font-weight: 600;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 0.9em;
    flex-wrap: wrap;
    font-size: 0.9em;
    margin-bottom: 0.5em;
  }
  .cal-key {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
  }
  .swatch {
    width: 1.4em;
    height: 1.4em;
    padding: 0;
    border: none;
    background: none;
  }
  .layout {
    display: flex;
    gap: 1em;
    align-items: flex-start;
  }
  .main {
    flex: 1;
    min-width: 0;
  }
  .tray {
    width: 12em;
    flex: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.4em 0.6em;
    max-height: 70vh;
    overflow: auto;
  }
  .tray h4 {
    margin: 0.2em 0;
  }
  .tray .muted,
  .muted {
    color: var(--muted);
    font-size: 0.85em;
  }
  .n {
    color: var(--muted);
    font-weight: normal;
  }
  .tray-item {
    background: var(--chip);
    border-radius: 4px;
    padding: 0.2em 0.4em;
    margin: 3px 0;
    cursor: grab;
    touch-action: none;
    user-select: none;
    font-size: 0.9em;
  }
  .tray-item.dragging {
    opacity: 0.5;
  }

  .cal {
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    background: var(--panel);
  }
  .row {
    display: grid;
    grid-template-columns: 3.4em repeat(var(--n), minmax(0, 1fr));
  }
  .gutter {
    font-size: 0.72em;
    color: var(--muted);
    text-align: right;
    padding-right: 0.4em;
  }
  .head {
    border-bottom: 1px solid var(--border);
  }
  .dayhead {
    background: none;
    border: none;
    border-left: 1px solid var(--border);
    padding: 0.3em 0;
    cursor: pointer;
    text-align: center;
  }
  .dayhead .wd {
    color: var(--muted);
    font-size: 0.85em;
  }
  .dayhead .dn {
    font-weight: 600;
  }
  .dayhead.today .dn {
    background: var(--accent);
    color: white;
    border-radius: 50%;
    padding: 0.05em 0.4em;
  }
  .strip {
    border-bottom: 1px solid var(--border);
    align-items: stretch;
  }
  .strip .gutter {
    padding-top: 0.3em;
  }
  .allday {
    border-left: 1px solid var(--border);
    min-height: 1.9em;
    padding: 2px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .allday.over,
  td.over {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .scroll {
    max-height: 62vh;
    overflow-y: auto;
  }
  .hours .hour {
    box-sizing: border-box;
    border-top: 1px solid transparent;
    position: relative;
    top: -0.6em;
  }
  .col {
    position: relative;
    border-left: 1px solid var(--border);
    background-image: repeating-linear-gradient(
      to bottom,
      var(--border) 0,
      var(--border) 1px,
      transparent 1px,
      transparent 48px
    );
  }
  .col.today {
    background-color: color-mix(in srgb, var(--accent) 5%, transparent);
  }
  .pos {
    position: absolute;
    box-sizing: border-box;
    padding-right: 1px;
  }
  .pos .item {
    height: 100%;
  }
  .now {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--danger);
    pointer-events: none;
  }
  .now::before {
    content: '';
    position: absolute;
    left: -4px;
    top: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }
  .ghost {
    position: absolute;
    left: 2px;
    right: 2px;
    border: 2px dashed var(--accent);
    border-radius: 4px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    font-size: 0.75em;
    padding: 1px 4px;
    pointer-events: none;
  }
  .create {
    position: absolute;
    left: 2px;
    right: 2px;
    z-index: 5;
  }
  .create input {
    width: 100%;
    box-sizing: border-box;
  }

  .item {
    box-sizing: border-box;
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.3em;
    overflow: hidden;
    padding: 1px 4px;
    border-radius: 4px;
    font-size: 0.8em;
    line-height: 1.25;
    cursor: pointer;
    touch-action: none;
    user-select: none;
    background: color-mix(in srgb, var(--c) 22%, var(--panel));
    border-left: 3px solid var(--c);
  }
  .item.task {
    background: var(--panel);
    border: 1.5px solid var(--c);
  }
  .item.due {
    border-style: dashed;
  }
  .item.rolled {
    border-style: dashed;
  }
  .roll {
    color: var(--danger);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .item.done .t {
    text-decoration: line-through;
    color: var(--muted);
  }
  .item.dragging,
  .chip.dragging {
    opacity: 0.45;
  }
  .item .at,
  .chip .at {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .item .loc {
    color: var(--muted);
    flex-basis: 100%;
  }
  .check {
    margin: 0;
    align-self: center;
  }
  .grip {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 6px;
    cursor: ns-resize;
    touch-action: none;
    background: linear-gradient(transparent, color-mix(in srgb, var(--c) 40%, transparent));
  }

  .month {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  .month th {
    font-weight: normal;
    color: var(--muted);
    font-size: 0.85em;
    padding: 0.2em;
  }
  .month td {
    border: 1px solid var(--border);
    vertical-align: top;
    height: 6.5em;
    padding: 2px 3px;
    overflow: hidden;
    font-size: 0.85em;
  }
  .month td.out {
    background: var(--code-bg);
    color: var(--muted);
  }
  .month td.today {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .num {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 0.8em;
    cursor: pointer;
    padding: 0;
  }
  .chip {
    border-left: 3px solid var(--c);
    background: color-mix(in srgb, var(--c) 20%, var(--panel));
    border-radius: 3px;
    padding: 0 3px;
    margin: 1px 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    touch-action: none;
    user-select: none;
  }
  .chip.task {
    background: var(--panel);
    border: 1px solid var(--c);
    border-left-width: 3px;
  }
  .chip.done {
    text-decoration: line-through;
    color: var(--muted);
  }
  .more {
    background: none;
    border: none;
    color: var(--link);
    cursor: pointer;
    font-size: 0.8em;
    padding: 0;
  }

  .popup-back {
    position: fixed;
    inset: 0;
    z-index: 20;
  }
  .popup {
    position: fixed;
    z-index: 21;
    top: 30%;
    left: 50%;
    transform: translateX(-50%);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.8em 1em;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.25);
    display: flex;
    flex-direction: column;
    gap: 0.3em;
    min-width: 14em;
  }
</style>
