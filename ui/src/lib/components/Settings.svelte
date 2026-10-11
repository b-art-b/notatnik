<script lang="ts">
  // Settings, organised by topic (Appearance, Dates & language, Tasks, Graph). Every change applies at once and
  // is confirmed inline, so there is no Save button to forget. Each pane says where its values are stored:
  // "this device" settings never leave the computer, "this graph" settings are synced with the notes.
  import { backend, type PageIconRuleView, type SettingsView } from '../api'
  import {
    app,
    applyLinkBrackets,
    applyTheme,
    errorMessage,
    LEFT_DEFAULT,
    openGraph,
    setLeftWidth,
    toast,
    type Theme,
  } from '../stores/app.svelte'
  import { DATE_FORMATS, formatDate, setDateFormat, setDateLocale, type DateFormat } from '../dates'
  import { currentZoom, setZoom, stepZoom } from '../zoom'
  import { loadIcons } from '../stores/icons.svelte'
  import { reloadOpenDocs } from '../stores/pages.svelte'
  import EmojiPicker from './EmojiPicker.svelte'
  import SettingRow from './settings/SettingRow.svelte'
  import Segmented from './settings/Segmented.svelte'

  const SAMPLE = '2026-10-03'
  /** Languages offered for dates (any BCP 47 tag works in the settings file). */
  const LOCALES: [string, string][] = [
    ['en-GB', 'English (UK)'],
    ['en-US', 'English (US)'],
    ['pl-PL', 'Polski'],
    ['de-DE', 'Deutsch'],
    ['fr-FR', 'Français'],
    ['es-ES', 'Español'],
    ['it-IT', 'Italiano'],
    ['nl-NL', 'Nederlands'],
    ['cs-CZ', 'Čeština'],
    ['uk-UA', 'Українська'],
    ['sv-SE', 'Svenska'],
  ]
  const FORMAT_NAMES: Record<DateFormat, string> = {
    full: 'Weekday, day, month, year',
    long: 'Day, full month, year',
    short: 'Numbers only',
    iso: 'ISO 8601',
    custom: 'Custom pattern',
  }
  const TABS = [
    { id: 'appearance', label: 'Appearance', icon: '◐' },
    { id: 'dates', label: 'Dates & language', icon: '▦' },
    { id: 'tasks', label: 'Tasks', icon: '☐' },
    { id: 'icons', label: 'Page icons', icon: '☺' },
    { id: 'graph', label: 'Graph', icon: '▤' },
  ] as const
  type TabId = (typeof TABS)[number]['id']

  let s = $state<SettingsView | null>(null)
  let tab = $state<TabId>('appearance')
  let path = $state('')
  let saved = $state('') // the inline confirmation ("Saved"), cleared after a moment
  let savedTimer: ReturnType<typeof setTimeout> | undefined
  let zoom = $state(currentZoom())

  // --- page icons: rules `property = value -> emoji`. Edited as a draft; saved when every row is complete ---
  let rules = $state<PageIconRuleView[]>([])
  let rulesLoaded = false
  let pickFor = $state<number | null>(null) // the row whose emoji picker is open
  let knownKeys = $state<string[]>([])
  let knownValues = $state<Record<string, string[]>>({})
  $effect(() => {
    if (s && !rulesLoaded) {
      rules = s.page_icons.map((r) => ({ ...r }))
      rulesLoaded = true
    }
  })
  $effect(() => {
    if (tab !== 'icons') return
    backend.call('page_property_keys', {}).then((k) => (knownKeys = k.map((x) => x.key)))
  })
  async function valuesOf(key: string) {
    const k = key.trim()
    // `type` always offers `journal`: journal pages have `type: journal` unless they say otherwise
    if (!k || knownValues[k] || (!knownKeys.includes(k) && k !== 'type')) return
    const g = await backend.call('page_groups', { key: k })
    const vals = g.flatMap((x) => (x.value === null ? [] : [x.value]))
    knownValues[k] = k === 'type' && !vals.includes('journal') ? [...vals, 'journal'] : vals
  }
  const complete = (r: PageIconRuleView) => r.key.trim() && r.value.trim() && r.icon.trim()
  function saveRules() {
    if (rules.every(complete)) void save({ page_icons: rules.map((r) => ({ ...r })) })
  }
  const hasJournalRule = $derived(
    rules.some((r) => r.key.trim() === 'type' && r.value.trim().toLowerCase() === 'journal'),
  )
  function addJournalRule() {
    rules.push({ key: 'type', value: 'journal', icon: '' })
    pickFor = rules.length - 1 // straight to the emoji picker
  }
  function addRule() {
    rules.push({ key: rules.at(-1)?.key ?? 'type', value: '', icon: '' })
  }
  function removeRule(i: number) {
    rules.splice(i, 1)
    pickFor = null
    saveRules()
  }
  function pickIcon(i: number, char: string) {
    rules[i].icon = char
    pickFor = null
    saveRules()
  }

  try {
    const t = localStorage.getItem('notatnik-settings-tab')
    if (TABS.some((x) => x.id === t)) tab = t as TabId
  } catch {
    /* default */
  }
  $effect(() => {
    try {
      localStorage.setItem('notatnik-settings-tab', tab)
    } catch {
      /* not remembered */
    }
  })
  $effect(() => {
    backend.call('get_settings', {}).then((v) => (s = v))
  })

  type Change = Partial<{
    theme: string
    date_locale: string
    date_format: string
    date_pattern: string
    link_brackets: boolean
    task_details: string
    block_properties: string
    week_start: string
    completed_log_limit: number
    priorities: string[]
    page_icons: PageIconRuleView[]
  }>

  // Saves run one after the other. A change may be a function: it is called when its turn comes, with the
  // settings as the previous save left them (editing three labels quickly must not lose two of them).
  let queue: Promise<void> = Promise.resolve()
  function save(change: Change | (() => Change)): Promise<void> {
    queue = queue.then(() => persist(typeof change === 'function' ? change() : change))
    return queue
  }

  async function persist(change: Change) {
    try {
      s = await backend.call('set_settings', {
        theme: null,
        date_locale: null,
        date_format: null,
        date_pattern: null,
        link_brackets: null,
        task_details: null,
        block_properties: null,
        week_start: null,
        completed_log_limit: null,
        priorities: null,
        page_icons: null,
        ...change,
      })
      setDateLocale(s.date_locale)
      setDateFormat(s.date_format, s.date_pattern)
      app.dateVersion++
      applyTheme(s.theme as Theme)
      applyLinkBrackets(s.link_brackets)
      app.taskDetails = s.task_details === 'compact' ? 'compact' : 'full'
      app.blockProps = s.block_properties === 'compact' ? 'compact' : 'full'
      app.graph = await backend.call('graph_info', {})
      if (change.page_icons) void loadIcons()
      // task views carry the labels: load the open pages again
      if (change.priorities) void reloadOpenDocs()
      toast('Settings saved')
      saved = 'Saved'
      clearTimeout(savedTimer)
      savedTimer = setTimeout(() => (saved = ''), 2500)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  function changeZoom(z: number) {
    setZoom(z)
    zoom = z
  }

  /** Arrow keys move between the categories (roving tabindex, as for any tablist). */
  function tabKey(e: KeyboardEvent) {
    const i = TABS.findIndex((t) => t.id === tab)
    let n = i
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') n = (i + 1) % TABS.length
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') n = (i - 1 + TABS.length) % TABS.length
    else return
    e.preventDefault()
    tab = TABS[n].id
    document.getElementById(`tab-${tab}`)?.focus()
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text)
      toast('Copied')
    } catch {
      toast('Could not copy', 'error')
    }
  }
</script>

<div class="settings">
  <header class="top">
    <h1>Settings</h1>
    <span class="saved" role="status" aria-live="polite">{saved ? `✓ ${saved}` : ''}</span>
  </header>

  <div class="layout">
    <div
      class="tabs"
      role="tablist"
      aria-label="Settings categories"
      aria-orientation="vertical"
      tabindex="-1"
    >
      {#each TABS as t (t.id)}
        <button
          role="tab"
          id={`tab-${t.id}`}
          aria-selected={tab === t.id}
          aria-controls={`pane-${t.id}`}
          tabindex={tab === t.id ? 0 : -1}
          class:on={tab === t.id}
          onclick={() => (tab = t.id)}
          onkeydown={tabKey}
        >
          <span class="ico" aria-hidden="true">{t.icon}</span>{t.label}
        </button>
      {/each}
    </div>

    {#if s}
      <div class="pane" role="tabpanel" id={`pane-${tab}`} aria-labelledby={`tab-${tab}`}>
        {#if tab === 'appearance'}
          <h2>Appearance</h2>
          <p class="scope">
            <span class="badge device">This device</span>
            Stored in <code>~/.config/notatnik/settings.toml</code> and never synced.
          </p>
          <SettingRow title="Theme" description="Light, dark, or follow your system setting.">
            <Segmented
              name="theme"
              legend="Theme"
              value={s.theme}
              options={[
                { value: 'system', label: 'System' },
                { value: 'light', label: 'Light' },
                { value: 'dark', label: 'Dark' },
              ]}
              onchange={(v) => save({ theme: v })}
            />
          </SettingRow>
          <SettingRow
            title="Zoom"
            description="Make everything larger or smaller. Also Ctrl +, Ctrl −, Ctrl 0 and Ctrl + mouse wheel."
          >
            <div class="stepper" role="group" aria-label="Zoom">
              <button aria-label="Zoom out" onclick={() => changeZoom(stepZoom(zoom, -1))}>−</button>
              <output aria-live="polite" data-testid="zoom-level">{Math.round(zoom * 100)}%</output>
              <button aria-label="Zoom in" onclick={() => changeZoom(stepZoom(zoom, 1))}>+</button>
              <button class="plain" disabled={zoom === 1} onclick={() => changeZoom(1)}>Reset</button>
            </div>
          </SettingRow>
          <SettingRow
            title="Sidebar width"
            description="Also drag the right edge of the sidebar (arrow keys when it has the focus, Home or double click resets). Ctrl + \\ hides the sidebar."
            labelFor="left-width"
          >
            <div class="stepper">
              <input
                id="left-width"
                type="range"
                min="180"
                max="560"
                step="10"
                value={app.leftWidth}
                oninput={(e) => setLeftWidth(Number((e.currentTarget as HTMLInputElement).value), false)}
                onchange={(e) => setLeftWidth(Number((e.currentTarget as HTMLInputElement).value))}
              />
              <output aria-live="polite" data-testid="left-width">{app.leftWidth}px</output>
              <button
                class="plain"
                disabled={app.leftWidth === LEFT_DEFAULT}
                onclick={() => setLeftWidth(LEFT_DEFAULT)}>Reset</button
              >
            </div>
          </SettingRow>
          <SettingRow
            title="Show brackets around links"
            description="Show [[ ]] around page links in rendered text. The brackets are always visible while you edit a block."
            labelFor="link-brackets"
          >
            <input
              id="link-brackets"
              class="switch"
              type="checkbox"
              role="switch"
              checked={s.link_brackets}
              onchange={(e) => save({ link_brackets: (e.currentTarget as HTMLInputElement).checked })}
            />
          </SettingRow>
          <SettingRow
            title="Task details"
            description="Compact shows a task's date, repeat and logbook as small indicators at the end of its line; the details appear when you hover or focus them (or tap on a touch screen). The agenda always shows everything."
          >
            <Segmented
              name="task-details"
              legend="Task details"
              value={s.task_details}
              options={[
                { value: 'full', label: 'Always shown' },
                { value: 'compact', label: 'Compact' },
              ]}
              onchange={(v) => save({ task_details: v })}
            />
          </SettingRow>
          <SettingRow
            title="Block properties"
            description="Compact replaces the property card under a block (client: Acme, type: partner, ...) with one small ⋯ indicator at the end of its line; the card opens when you hover or focus it (or tap). The agenda, query results and linked references always show everything."
          >
            <Segmented
              name="block-properties"
              legend="Block properties"
              value={s.block_properties}
              options={[
                { value: 'full', label: 'Always shown' },
                { value: 'compact', label: 'Compact' },
              ]}
              onchange={(v) => save({ block_properties: v })}
            />
          </SettingRow>
        {:else if tab === 'dates'}
          <h2>Dates &amp; language</h2>
          <p class="scope">
            <span class="badge device">This device</span>
            How dates are <em>shown</em>. Files and date pickers always use <code>YYYY-MM-DD</code>.
          </p>
          <SettingRow
            title="Language"
            description="Used for month and weekday names."
            labelFor="date-language"
          >
            <select
              id="date-language"
              value={s.date_locale}
              onchange={(e) => save({ date_locale: (e.currentTarget as HTMLSelectElement).value })}
            >
              {#each LOCALES as [tag, name] (tag)}<option value={tag}>{name} ({tag})</option>{/each}
            </select>
          </SettingRow>
          <SettingRow title="Format" description="Pick a style, or write your own pattern.">
            <div class="formats" role="radiogroup" aria-label="Date format">
              {#each DATE_FORMATS as f (f)}
                <label class="fmt" class:on={s.date_format === f}>
                  <input
                    type="radio"
                    name="date-format"
                    value={f}
                    checked={s.date_format === f}
                    onchange={() => save({ date_format: f })}
                  />
                  <span class="fname">{FORMAT_NAMES[f]}</span>
                  <span class="fex"
                    >{f === 'custom'
                      ? formatDate(SAMPLE, s.date_locale, 'custom', s.date_pattern)
                      : formatDate(SAMPLE, s.date_locale, f)}</span
                  >
                </label>
              {/each}
            </div>
          </SettingRow>
          {#if s.date_format === 'custom'}
            <div class="custom">
              <label for="date-pattern">Pattern</label>
              <input
                id="date-pattern"
                value={s.date_pattern}
                maxlength="80"
                placeholder="dddd, D MMMM YYYY"
                onchange={(e) => save({ date_pattern: (e.currentTarget as HTMLInputElement).value })}
              />
              <details>
                <summary>Pattern reference</summary>
                <table class="tokens">
                  <tbody>
                    <tr><td><code>YYYY</code> <code>YY</code></td><td>year: 2026, 26</td></tr>
                    <tr><td><code>MMMM</code> <code>MMM</code></td><td>month name: October, Oct</td></tr>
                    <tr><td><code>MM</code> <code>M</code></td><td>month number: 03, 3</td></tr>
                    <tr><td><code>DD</code> <code>D</code></td><td>day: 03, 3</td></tr>
                    <tr><td><code>dddd</code> <code>ddd</code></td><td>weekday: Saturday, Sat</td></tr>
                    <tr><td><code>[text]</code></td><td>literal text, e.g. <code>[week]</code></td></tr>
                  </tbody>
                </table>
                <p class="muted">
                  Examples: <code>D.MM.YY</code>, <code>YYYY/MM/DD (ddd)</code>. Letters that are tokens are
                  replaced even inside words, so write plain text in brackets. Full reference:
                  <code>docs/date-formats.md</code>.
                </p>
              </details>
            </div>
          {/if}
          <p class="preview">
            Preview:
            <strong data-testid="date-preview"
              >{formatDate(SAMPLE, s.date_locale, s.date_format as DateFormat, s.date_pattern)}</strong
            >
          </p>
          <hr />
          <SettingRow title="Week starts on" description="Used by calendars and the agenda.">
            <Segmented
              name="week-start"
              legend="Week starts on"
              value={s.week_start}
              options={[
                { value: 'monday', label: 'Monday' },
                { value: 'sunday', label: 'Sunday' },
              ]}
              onchange={(v) => save({ week_start: v })}
            />
          </SettingRow>
          <p class="scope inline">
            <span class="badge graph">This graph</span> The first day of the week belongs to the graph and is synced
            with it.
          </p>
        {:else if tab === 'tasks'}
          <h2>Tasks</h2>
          <p class="scope">
            <span class="badge graph">This graph</span>
            Stored in <code>.notatnik/config.toml</code> and synced with your notes.
          </p>
          <SettingRow
            title="Completion dates to keep"
            description="A repeating task logs the dates it was completed. Keep only the most recent ones; 0 keeps them all."
            labelFor="log-limit"
          >
            <span class="num"
              ><input
                id="log-limit"
                type="number"
                min="0"
                value={s.completed_log_limit}
                onchange={(e) =>
                  save({ completed_log_limit: Number((e.currentTarget as HTMLInputElement).value) })}
              />
              <span class="unit">dates</span></span
            >
          </SettingRow>
          <SettingRow
            title="Priorities"
            description="Your priority labels, most important first (one to nine). They show after the task marker as [#A]. A task has no priority until you set one (/priority, or click the label). In files the property is the label; high, medium and low still work."
          >
            <div class="stepper prios" role="group" aria-label="Priority labels">
              {#each s.priorities as label, i (i)}
                <input
                  class="prio-input"
                  aria-label={`Label of priority ${i + 1}`}
                  maxlength="8"
                  value={label}
                  onchange={(e) => {
                    const value = (e.currentTarget as HTMLInputElement).value.trim()
                    void save(() => {
                      const next = [...(s?.priorities ?? [])]
                      next[i] = value
                      return { priorities: next }
                    })
                  }}
                />
              {/each}
              <button
                class="plain"
                disabled={s.priorities.length >= 9}
                onclick={() =>
                  void save(() => {
                    const cur = s?.priorities ?? []
                    const spare = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J'].find(
                      (l) => !cur.some((c) => c.toLowerCase() === l.toLowerCase()),
                    )
                    return { priorities: [...cur, spare ?? String(cur.length + 1)] }
                  })}>+ Add</button
              >
              <button
                class="plain"
                disabled={s.priorities.length <= 1}
                onclick={() => save(() => ({ priorities: (s?.priorities ?? []).slice(0, -1) }))}
                >− Remove last</button
              >
              <button
                class="plain"
                disabled={s.priorities.join(',') === 'A,B,C'}
                onclick={() => save({ priorities: ['A', 'B', 'C'] })}>Reset</button
              >
            </div>
          </SettingRow>
        {:else if tab === 'icons'}
          <h2>Page icons</h2>
          <p class="scope">
            <span class="badge graph">This graph</span>
            Stored in <code>.notatnik/config.toml</code> and synced with your notes.
          </p>
          <p class="intro">
            Give every page with a property value an emoji: <em>type = person</em> → 👤. The icon appears next
            to the page in lists, links and titles. A page can also set its own icon with
            <code>icon: 🏢</code> in its properties, which wins over these rules. Journal days have
            <em>type = journal</em> unless their properties say otherwise, so a rule for it sets the icon of every
            day (the calendar is shown until you pick one).
          </p>
          <div class="rules" data-testid="icon-rules">
            {#each rules as r, i (i)}
              <div class="rule" data-testid="icon-rule">
                <input
                  aria-label="Property"
                  list="icon-keys"
                  placeholder="property, e.g. type"
                  bind:value={r.key}
                  onchange={saveRules}
                  onfocus={() => valuesOf(r.key)}
                />
                <span class="eq">=</span>
                <input
                  aria-label="Value"
                  list={`icon-values-${i}`}
                  placeholder="value, e.g. person"
                  bind:value={r.value}
                  onchange={saveRules}
                  onfocus={() => valuesOf(r.key)}
                />
                <datalist id={`icon-values-${i}`}
                  >{#each knownValues[r.key.trim()] ?? [] as v (v)}<option value={v}
                    ></option>{/each}</datalist
                >
                <span class="arrow">→</span>
                <span class="pick">
                  <button
                    type="button"
                    class="emoji"
                    aria-label={r.icon ? `Icon ${r.icon}` : 'Choose an icon'}
                    aria-expanded={pickFor === i}
                    onclick={() => (pickFor = pickFor === i ? null : i)}>{r.icon || '＋'}</button
                  >
                  {#if pickFor === i}
                    <div class="pop">
                      <EmojiPicker onpick={(c) => pickIcon(i, c)} oncancel={() => (pickFor = null)} />
                    </div>
                  {/if}
                </span>
                <button
                  type="button"
                  class="plain"
                  aria-label={`Remove rule ${i + 1}`}
                  title="Remove this rule"
                  onclick={() => removeRule(i)}>✕</button
                >
              </div>
            {:else}
              <p class="muted">No rules yet.</p>
            {/each}
            <datalist id="icon-keys"
              >{#each knownKeys as k (k)}<option value={k}></option>{/each}</datalist
            >
            <button type="button" onclick={addRule}>Add a rule</button>
            {#if !hasJournalRule}
              <button type="button" onclick={addJournalRule} data-testid="add-journal-rule"
                >Add an icon for journal days</button
              >
            {/if}
            {#if rules.some((r) => !complete(r))}
              <p class="muted">A rule is saved when its property, value and icon are all filled in.</p>
            {/if}
          </div>
        {:else}
          <h2>Graph</h2>
          <p class="scope">
            <span class="badge graph">This graph</span> The folder of Markdown files you are working in.
          </p>
          <SettingRow title="Folder" description="Your notes are plain Markdown files here.">
            <span class="path"
              ><code data-testid="graph-folder">{s.graph_root}</code>
              <button class="plain" onclick={() => s && copy(s.graph_root)}>Copy</button></span
            >
          </SettingRow>
          <SettingRow
            title="Open or create another graph"
            description="Enter the path of a folder. If it has no graph yet, one is created there."
            labelFor="graph-path"
          >
            <span class="open"
              ><input id="graph-path" bind:value={path} placeholder="/path/to/folder" />
              <button disabled={!path.trim()} onclick={() => openGraph(path.trim(), true)}>Open</button></span
            >
          </SettingRow>
        {/if}
      </div>
    {:else}
      <p class="muted">Loading…</p>
    {/if}
  </div>
</div>

<style>
  .settings {
    max-width: 52em;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 1em;
  }
  .saved {
    color: var(--muted);
    font-size: 0.85em;
    min-height: 1.2em;
  }
  .layout {
    display: grid;
    grid-template-columns: 13em 1fr;
    gap: 2em;
    align-items: start;
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: 0.15em;
    position: sticky;
    top: 1em;
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: 0.6em;
    text-align: left;
    border: none;
    background: none;
    color: inherit;
    padding: 0.5em 0.8em;
    border-radius: 8px;
    cursor: pointer;
  }
  .tabs button:hover {
    background: var(--chip);
  }
  .tabs button.on {
    background: var(--selection);
    font-weight: 600;
  }
  .ico {
    width: 1.2em;
    text-align: center;
    color: var(--muted);
  }
  .pane h2 {
    margin: 0.2em 0 0.3em;
  }
  .scope {
    margin: 0 0 0.6em;
    color: var(--muted);
    font-size: 0.85em;
  }
  .scope.inline {
    margin-top: 0.4em;
  }
  .badge {
    display: inline-block;
    border-radius: 6px;
    padding: 0 0.5em;
    margin-right: 0.4em;
    font-size: 0.9em;
    font-weight: 600;
    color: var(--text);
    background: var(--chip);
  }
  .badge.graph {
    background: var(--selection);
  }
  select,
  input:not([type='radio']):not([type='checkbox']) {
    font: inherit;
    padding: 0.3em 0.5em;
  }
  .switch {
    appearance: none;
    width: 2.6em;
    height: 1.45em;
    border-radius: 1em;
    background: var(--border);
    position: relative;
    cursor: pointer;
    transition: background 0.15s;
    margin: 0;
  }
  .switch::after {
    content: '';
    position: absolute;
    top: 0.15em;
    left: 0.15em;
    width: 1.15em;
    height: 1.15em;
    border-radius: 50%;
    background: var(--panel);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.3);
    transition: transform 0.15s;
  }
  .switch:checked {
    background: var(--accent);
  }
  .switch:checked::after {
    transform: translateX(1.15em);
  }
  .switch:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .stepper {
    display: inline-flex;
    align-items: center;
    gap: 0.4em;
  }
  .stepper button:not(.plain) {
    width: 2em;
    height: 2em;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    color: inherit;
    cursor: pointer;
  }
  .stepper output {
    min-width: 3.2em;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .prio-input {
    width: 4.2em;
    text-align: center;
  }
  .plain {
    border: none;
    background: none;
    color: var(--link);
    cursor: pointer;
  }
  .plain:disabled {
    color: var(--muted);
    cursor: default;
  }
  .formats {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
    min-width: 19em;
  }
  .fmt {
    position: relative;
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0 1em;
    align-items: baseline;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4em 0.7em;
    cursor: pointer;
  }
  .fmt.on {
    border-color: var(--accent);
    background: var(--selection);
  }
  .fmt input {
    position: absolute;
    opacity: 0;
    inset: 0;
    cursor: pointer;
  }
  .fmt:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .fname {
    font-size: 0.85em;
    color: var(--muted);
  }
  .fex {
    font-weight: 600;
  }
  .custom {
    margin: 0.2em 0 0.6em;
    display: grid;
    gap: 0.3em;
  }
  .custom input {
    max-width: 22em;
  }
  .tokens td {
    padding: 0.1em 1em 0.1em 0;
  }
  .preview {
    margin: 0.6em 0;
  }
  .intro {
    max-width: 38em;
    margin: 0.2em 0 1em;
  }
  .rules {
    display: flex;
    flex-direction: column;
    gap: 0.5em;
    align-items: flex-start;
  }
  .rule {
    display: flex;
    align-items: center;
    gap: 0.5em;
    flex-wrap: wrap;
  }
  .rule input {
    width: 11em;
  }
  .eq,
  .arrow {
    color: var(--muted);
  }
  .pick {
    position: relative;
  }
  .emoji {
    width: 2.4em;
    height: 2.2em;
    font-size: 1.2em;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    cursor: pointer;
  }
  .pop {
    position: absolute;
    z-index: 30;
    top: 100%;
    left: 0;
    margin-top: 0.3em;
  }
  hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 0.8em 0;
  }
  .num,
  .open,
  .path {
    display: inline-flex;
    align-items: center;
    gap: 0.5em;
    max-width: 100%;
  }
  .num input {
    width: 5em;
  }
  .unit {
    color: var(--muted);
  }
  .path code {
    overflow-wrap: anywhere;
  }
  .open input {
    min-width: 16em;
  }
  @media (max-width: 760px) {
    .layout {
      grid-template-columns: 1fr;
      gap: 0.8em;
    }
    .tabs {
      flex-direction: row;
      flex-wrap: wrap;
      position: static;
    }
  }
</style>
