<script lang="ts">
  // Settings, organised by topic (Appearance, Dates & language, Tasks, Graph). Every change applies at once and
  // is confirmed inline, so there is no Save button to forget. Each pane says where its values are stored:
  // "this device" settings never leave the computer, "this graph" settings are synced with the notes.
  import {
    backend,
    type BackupChange,
    type BackupEncryption,
    type BackupFileDiff,
    type BackupList,
    type DeviceRestorePreview,
    type PageIconRuleView,
    type SettingsView,
  } from '../api'
  import {
    app,
    applyLinkBrackets,
    applyTheme,
    errorMessage,
    LEFT_DEFAULT,
    openGraph,
    setLeftWidth,
    setPlacement,
    toast,
    type Theme,
  } from '../stores/app.svelte'
  import { DATE_FORMATS, formatDate, setDateFormat, setDateLocale, type DateFormat } from '../dates'
  import { currentZoom, setZoom, stepZoom } from '../zoom'
  import { loadIcons } from '../stores/icons.svelte'
  import { reloadOpenDocs } from '../stores/pages.svelte'
  import EmojiPicker from './EmojiPicker.svelte'
  import SettingRow from './settings/SettingRow.svelte'
  import SyncPane from './settings/SyncPane.svelte'
  import FileSources from './settings/FileSources.svelte'
  import CalendarPane from './settings/CalendarPane.svelte'
  import UnusedImages from './settings/UnusedImages.svelte'
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
    { id: 'templates', label: 'Templates', icon: '⎘' },
    { id: 'sync', label: 'Sync', icon: '☁' },
    { id: 'calendar', label: 'Calendar', icon: '📅' },
    { id: 'files', label: 'Linked files', icon: '📎' },
    { id: 'backup', label: 'Backup', icon: '⤓' },
    { id: 'graph', label: 'Graph', icon: '▤' },
  ] as const
  type TabId = (typeof TABS)[number]['id']

  let s = $state<SettingsView | null>(null)
  let tab = $state<TabId>('appearance')
  let path = $state('')
  let saved = $state('') // the inline confirmation ("Saved"), cleared after a moment
  let savedTimer: ReturnType<typeof setTimeout> | undefined
  let zoom = $state(currentZoom())

  // --- templates: one file per page type (PR-10) ---
  type TemplateRow = { name: string; fields: string[]; blocks: number }
  let templates = $state<TemplateRow[]>([])
  let deletedTpls = $state<{ id: string; name: string; deleted_ms: number }[]>([])
  let nsTemplates = $state<{ namespace: string; template: string }[]>([])
  let nsLoaded = false
  $effect(() => {
    if (tab !== 'templates' || !s) return
    if (!nsLoaded) {
      nsTemplates = s.namespace_templates.map((r) => ({ ...r }))
      nsLoaded = true
    }
    backend.call('list_templates', {}).then(
      (r) => (templates = r),
      (e) => toast(errorMessage(e), 'error'),
    )
    backend.call('deleted_templates', {}).then((r) => (deletedTpls = r))
  })
  async function deleteTemplate(name: string) {
    try {
      templates = await backend.call('delete_template', { name })
      deletedTpls = await backend.call('deleted_templates', {})
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function restoreTemplate(id: string, name: string) {
    try {
      templates = await backend.call('restore_template', { id, name })
      deletedTpls = await backend.call('deleted_templates', {})
      toast(`Restored the template “${name}”`)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  function saveNsTemplates() {
    // a row counts once both fields are filled; the others stay as a draft
    const rows = nsTemplates.filter((r) => r.namespace.trim() && r.template.trim())
    void save({ namespace_templates: rows.map((r) => ({ ...r })) })
  }

  // --- backup: snapshots of the graph in a folder outside it (BK-1..BK-5) ---
  let bk = $state<BackupList | null>(null)
  let bkBusy = $state(false)
  let bkDir = $state('') // the folder field is a draft until it is changed (on blur / Enter)
  let restoreOf = $state<string | null>(null) // the snapshot whose restore form is open
  let restoreTarget = $state('')
  let restorePage = $state('')
  let bkMirror = $state('') // the second folder, a draft like the first
  let devicePreview = $state<DeviceRestorePreview | null>(null) // the device settings of the open restore form
  $effect(() => {
    if (tab !== 'backup' || !s) return
    bkDir = s.backup_dir
    bkMirror = s.backup_mirror_dir
    void loadBackups()
    void loadEnc()
  })
  async function loadBackups() {
    try {
      bk = await backend.call('list_backups', {})
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function backupNow() {
    bkBusy = true
    try {
      bk = await backend.call('backup_now', {})
      toast('Backup saved')
    } catch (e) {
      toast(errorMessage(e), 'error')
    } finally {
      bkBusy = false
    }
  }
  async function setBackupDir(dir: string) {
    await save({ backup_dir: dir.trim() })
    if (s) bkDir = s.backup_dir
    await loadBackups()
  }
  async function setMirrorDir(dir: string) {
    await save({ backup_mirror_dir: dir.trim() })
    if (s) bkMirror = s.backup_mirror_dir
    await loadBackups()
  }
  // --- encryption of the backups (EN-2, EN-5, EN-6): the keys live in memory only and lock after 15 idle minutes ---
  let enc = $state<BackupEncryption | null>(null)
  let encForm = $state<'' | 'setup' | 'unlock' | 'password' | 'recovery' | 'reset'>('')
  let encPw = $state('')
  let encPw2 = $state('')
  let encOld = $state('')
  let recoveryShown = $state<string | null>(null) // the recovery key, right after it was made: shown once
  let recoverySaved = $state(false)
  // the password was forgotten and a new vault is being made (EN-22): its recovery key has to be confirmed
  let encResetting = $state(false)
  let encUnderstood = $state(false)
  async function loadEnc() {
    try {
      enc = await backend.call('backup_encryption_status', {})
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  function closeEncForm() {
    encForm = ''
    encPw = encPw2 = encOld = ''
    encUnderstood = false
  }
  const encPasswordOk = () => encPw.length >= 8 && encPw === encPw2
  async function encRun(work: () => Promise<void>) {
    try {
      await work()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  const encSetup = () =>
    encRun(async () => {
      const r = await backend.call('backup_encryption_setup', { password: encPw })
      recoveryShown = r.recovery_key
      recoverySaved = false
      closeEncForm()
      await loadEnc()
    })
  const encReset = () =>
    encRun(async () => {
      const r = await backend.call('backup_encryption_setup', { password: encPw, reset: true })
      recoveryShown = r.recovery_key
      recoverySaved = false
      encResetting = true
      closeEncForm()
      await loadEnc()
    })
  const encConfirm = () =>
    encRun(async () => {
      enc = await backend.call('backup_encryption_confirm', {})
      encResetting = false
      recoveryShown = null
      s = await backend.call('get_settings', {})
      await loadBackups()
      toast('New backups are encrypted')
    })
  const encUnlock = () =>
    encRun(async () => {
      enc = await backend.call('backup_unlock', { secret: encPw })
      closeEncForm()
      if (restoreOf) void loadChanges(restoreOf)
    })
  const encLock = () =>
    encRun(async () => {
      enc = await backend.call('backup_lock', {})
      changes = null
    })
  const encChange = () =>
    encRun(async () => {
      enc = await backend.call('backup_encryption_change_password', {
        old_password: encOld,
        new_password: encPw,
      })
      closeEncForm()
      toast('Password changed. Backups already made still open with the old password too.')
    })
  const encNewRecovery = () =>
    encRun(async () => {
      const r = await backend.call('backup_encryption_new_recovery_key', { password: encOld })
      recoveryShown = r.recovery_key
      recoverySaved = true // the key is already in use: there is nothing to confirm, only to save
      closeEncForm()
    })
  const encExisting = () =>
    encRun(async () => {
      enc = await backend.call('backup_encrypt_existing', {})
      await loadBackups()
      toast('Existing backups are encrypted')
    })

  // the files that differ between the open snapshot and now, and the lines of those the user opened
  let changes = $state<BackupChange[] | null>(null)
  let diffs = $state<Record<string, BackupFileDiff>>({})
  let openPaths = $state<Record<string, boolean>>({})
  function openRestore(name: string) {
    restoreOf = restoreOf === name ? null : name
    restoreTarget = ''
    restorePage = ''
    devicePreview = null
    changes = null
    diffs = {}
    openPaths = {}
    const snap = bk?.backups.find((x) => x.name === name)
    if (restoreOf && (!snap?.encrypted || enc?.unlocked)) void loadChanges(name)
  }
  async function loadChanges(name: string) {
    try {
      const c = await backend.call('backup_compare', { name })
      if (restoreOf === name) changes = c.changes
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function showDiff(name: string, path: string, on: boolean) {
    openPaths[path] = on
    if (!on || diffs[path]) return
    try {
      diffs[path] = await backend.call('backup_file_diff', { name, path })
    } catch (e) {
      openPaths[path] = false
      toast(errorMessage(e), 'error')
    }
  }
  async function showAll(name: string, on: boolean) {
    for (const c of changes ?? []) if (!c.binary) await showDiff(name, c.path, on)
  }
  const CHANGE_LABEL: Record<string, string> = {
    changed: 'changed',
    only_snapshot: 'deleted since',
    only_now: 'new since',
  }
  async function compareDevice(name: string) {
    try {
      devicePreview = await backend.call('backup_device_preview', { name })
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function restoreDevice(name: string) {
    try {
      s = await backend.call('restore_device_settings', { name })
      setDateLocale(s.date_locale)
      setDateFormat(s.date_format, s.date_pattern)
      app.dateVersion++
      applyTheme(s.theme as Theme)
      applyLinkBrackets(s.link_brackets)
      toast('Device settings restored. The settings they replaced are kept next to the file.')
      devicePreview = null
      restoreOf = null
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function restoreFolder(name: string) {
    try {
      await backend.call('restore_backup', { name, target: restoreTarget.trim() })
      toast(`Restored to ${restoreTarget.trim()}. Open that folder as a graph to look around.`)
      restoreOf = null
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function restoreOnePage(name: string) {
    try {
      await backend.call('restore_backup_page', { name, page: restorePage.trim() })
      toast(`Restored ${restorePage.trim()}. The version it replaced is in a new backup.`)
      restoreOf = null
      await loadBackups()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  const mb = (bytes: number) =>
    bytes < 1024 * 1024
      ? `${Math.max(1, Math.round(bytes / 1024))} KB`
      : `${(bytes / 1024 / 1024).toFixed(1)} MB`

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
    backup_dir: string
    backup_auto: boolean
    backup_compression: string
    backup_mirror_dir: string
    backup_encrypt: boolean
    templates_auto: boolean
    open_last_graph: boolean
    journal_template: string
    namespace_templates: { namespace: string; template: string }[]
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
        backup_dir: null,
        backup_auto: null,
        backup_compression: null,
        backup_mirror_dir: null,
        backup_encrypt: null,
        templates_auto: null,
        open_last_graph: null,
        journal_template: null,
        namespace_templates: null,
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
            title="References"
            description="Where a page's linked and unlinked references go: below the page, or in a panel at the right (Ctrl+Shift+\\ hides it). Shown below the page when the window is narrower than about 1180 px. This device."
          >
            <Segmented
              name="place-refs"
              legend="References"
              value={app.placement.refs}
              options={[
                { value: 'bottom', label: 'Below the page' },
                { value: 'side', label: 'Side panel' },
              ]}
              onchange={(v) => setPlacement('refs', v === 'side' ? 'side' : 'bottom')}
            />
          </SettingRow>
          <SettingRow
            title="Page tasks"
            description="Where the tasks of a page (and of the pages below it) go. Same choices as for references; the two can be in different places."
          >
            <Segmented
              name="place-tasks"
              legend="Page tasks"
              value={app.placement.tasks}
              options={[
                { value: 'bottom', label: 'Below the page' },
                { value: 'side', label: 'Side panel' },
              ]}
              onchange={(v) => setPlacement('tasks', v === 'side' ? 'side' : 'bottom')}
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
        {:else if tab === 'templates'}
          <h2>Templates</h2>
          <p class="scope">
            <span class="badge graph">This graph</span>
            Template files live in the <code>templates</code> folder of the graph and sync with your notes. Whether
            they are applied automatically is a setting of this device.
          </p>
          <p class="intro">
            A template belongs to a page type: <code>templates/person.md</code> is used by every page with
            <code>type: person</code>. Its properties are the fields a page of that type should have (an empty
            value shows the field empty), its blocks are the starting content. The placeholders below are
            filled in when a template is used.
          </p>
          <p class="intro">
            <code>/template</code> in a block inserts the blocks of a template there.
          </p>
          <p class="intro">
            When you set <code>type</code> on a page, the properties of its template that the page lacks are
            added. Values you already set are never changed, and the blocks are added only to a page that has
            none. An existing page can pick up new template fields with <em>Add missing from template</em> in its
            properties.
          </p>
          <p class="intro">
            To make a template, set up one page the way you want (its <code>type</code>, properties and
            blocks) and click <em>Save as template…</em> in its properties. Saving again replaces the template after
            asking.
          </p>
          <h3>Placeholders</h3>
          <table class="tokens" data-testid="placeholders">
            <thead>
              <tr><th>Placeholder</th><th>Where</th><th>Becomes</th></tr>
            </thead>
            <tbody>
              <tr>
                <td><code>{'{{title}}'}</code></td><td>templates, queries</td><td
                  >the last part of the page name (<code>Jan</code> for <code>people/Jan</code>)</td
                >
              </tr>
              <tr>
                <td><code>{'{{namespace}}'}</code></td><td>templates, queries</td><td
                  >the part before it (<code>people</code>)</td
                >
              </tr>
              <tr>
                <td><code>{'{{date}}'}</code></td><td>templates, queries</td><td
                  >today; for a new journal day, that day (<code>2026-10-11</code>)</td
                >
              </tr>
              <tr>
                <td><code>{'{{cursor}}'}</code></td><td>templates</td><td
                  >removed (reserved for the cursor position)</td
                >
              </tr>
              <tr>
                <td><code>{'{{page}}'}</code></td><td>queries</td><td
                  >a link to the page the query block is on: <code>prop.partner: {'{{page}}'}</code>. Not
                  available in the agenda or a saved query.</td
                >
              </tr>
            </tbody>
          </table>
          <h3>Your templates</h3>
          {#if templates.length === 0}
            <p class="intro" data-testid="no-templates">None yet.</p>
          {:else}
            <ul class="snaps" data-testid="template-list">
              {#each templates as t (t.name)}
                <li>
                  <strong>{t.name}</strong>
                  <span class="muted"
                    >{t.fields.length ? t.fields.join(', ') : 'no fields'} · {t.blocks} blocks</span
                  >
                  <button class="plain" onclick={() => deleteTemplate(t.name)}>Delete</button>
                </li>
              {/each}
            </ul>
          {/if}
          {#if deletedTpls.length}
            <h3>Deleted templates</h3>
            <p class="intro">Kept for 30 days on this device.</p>
            <ul class="snaps" data-testid="deleted-templates">
              {#each deletedTpls as d (d.id + d.name)}
                <li>
                  <strong>{d.name}</strong>
                  <span class="muted">deleted {new Date(d.deleted_ms).toLocaleString()}</span>
                  <button class="plain" onclick={() => restoreTemplate(d.id, d.name)}>Restore</button>
                </li>
              {/each}
            </ul>
          {/if}
          <h3>New pages and days</h3>
          <p class="intro">
            A new page below a namespace starts from its template (properties, <code>type</code> and blocks), and
            so does a new journal day. Your first block comes after the template's.
          </p>
          <SettingRow
            title="Journal template"
            description="Used for every new journal day."
            labelFor="journal-template"
          >
            <select
              id="journal-template"
              value={s.journal_template}
              onchange={(e) => save({ journal_template: (e.currentTarget as HTMLSelectElement).value })}
            >
              <option value="">None</option>
              {#each templates as t (t.name)}<option value={t.name}>{t.name}</option>{/each}
            </select>
          </SettingRow>
          <div class="rules" data-testid="namespace-templates">
            {#each nsTemplates as r, i (i)}
              <div class="rule">
                <input
                  aria-label="Namespace"
                  placeholder="namespace, e.g. people"
                  bind:value={r.namespace}
                  onchange={saveNsTemplates}
                />
                <select aria-label="Template" bind:value={r.template} onchange={saveNsTemplates}>
                  <option value="">choose a template…</option>
                  {#each templates as t (t.name)}<option value={t.name}>{t.name}</option>{/each}
                </select>
                <button
                  class="plain"
                  aria-label="Remove this namespace template"
                  onclick={() => {
                    nsTemplates.splice(i, 1)
                    saveNsTemplates()
                  }}>✕</button
                >
              </div>
            {/each}
            <button class="plain" onclick={() => nsTemplates.push({ namespace: '', template: '' })}
              >Add a namespace template</button
            >
          </div>
          <SettingRow
            title="Apply templates automatically"
            description="When a type is set, and to new pages below a namespace or new journal days. Off: nothing is added automatically; the button in the page properties and /template still work."
            labelFor="templates-auto"
          >
            <input
              id="templates-auto"
              class="switch"
              type="checkbox"
              role="switch"
              checked={s.templates_auto}
              onchange={(e) => save({ templates_auto: (e.currentTarget as HTMLInputElement).checked })}
            />
          </SettingRow>
        {:else if tab === 'sync'}
          <SyncPane />
        {:else if tab === 'calendar'}
          <CalendarPane />
        {:else if tab === 'files'}
          <FileSources />
        {:else if tab === 'backup'}
          <h2>Backup</h2>
          <p class="scope">
            <span class="badge">This device</span>
            Snapshots are zip files of this graph's folder and of this device's settings. They are kept outside
            the graph, so they are never synced.
          </p>
          <p class="intro">
            Sync is not a backup: it copies a deleted or damaged page to every device. A snapshot is a full
            copy at one moment. Keep the folder on another disk, or in a folder that another tool copies
            elsewhere.
          </p>
          <SettingRow
            title="Automatic backups"
            description="One snapshot when the app starts and then daily while it is open, only when something changed. Kept: the last 7 days, 4 weeks and 6 months."
            labelFor="backup-auto"
          >
            <input
              id="backup-auto"
              class="switch"
              type="checkbox"
              role="switch"
              checked={s.backup_auto}
              onchange={(e) => save({ backup_auto: (e.currentTarget as HTMLInputElement).checked })}
            />
          </SettingRow>
          <SettingRow
            title="Backup folder"
            description="An absolute path outside the graph folder. Leave it empty for the default folder."
            labelFor="backup-dir"
          >
            <span class="open"
              ><input
                id="backup-dir"
                bind:value={bkDir}
                placeholder={bk?.default_dir ?? ''}
                onchange={() => setBackupDir(bkDir)}
              />
              <button class="plain" disabled={!s.backup_dir} onclick={() => setBackupDir('')}
                >Use default</button
              ></span
            >
          </SettingRow>
          <SettingRow
            title="Compression"
            description="Compressed snapshots are about a third of the size. Not compressed keeps every file as it is inside the zip: bigger, but fine when the target compresses or deduplicates itself. Both are ordinary zip files. Applies to new snapshots."
          >
            <Segmented
              name="backup-compression"
              legend="Compression"
              value={s.backup_compression}
              options={[
                { value: 'compressed', label: 'Compressed' },
                { value: 'none', label: 'Not compressed' },
              ]}
              onchange={(v) => save({ backup_compression: v })}
            />
          </SettingRow>
          <SettingRow
            title="Encryption"
            description="Seal backups with a password so that a backup disk or the second folder only ever holds unreadable files. Backups are written without asking for the password; it is needed to read one. Your notes in the graph folder stay plain text."
          >
            {#if enc?.configured}
              <span class="open">
                <input
                  class="switch"
                  type="checkbox"
                  role="switch"
                  aria-label="Encrypt new backups"
                  checked={s.backup_encrypt}
                  onchange={(e) => save({ backup_encrypt: (e.currentTarget as HTMLInputElement).checked })}
                />
                <span class="muted" data-testid="enc-state">{enc.unlocked ? 'Unlocked' : 'Locked'}</span>
                {#if enc.unlocked}
                  <button class="plain" onclick={encLock}>Lock</button>
                {:else}
                  <button class="plain" onclick={() => (encForm = encForm === 'unlock' ? '' : 'unlock')}
                    >Unlock…</button
                  >
                {/if}
              </span>
            {:else}
              <button data-testid="enc-setup" onclick={() => (encForm = encForm === 'setup' ? '' : 'setup')}
                >Set up…</button
              >
            {/if}
          </SettingRow>
          {#if recoveryShown}
            <div class="recovery" role="group" aria-label="Recovery key" data-testid="recovery-box">
              <strong>Your recovery key</strong>
              <p class="muted">
                It opens your backups if you forget the password. It is shown only now. Write it down or put
                it in a password manager, and keep it away from the backups. Without the password and this key
                the backups cannot be read by anyone.
              </p>
              <code class="key" data-testid="recovery-key">{recoveryShown}</code>
              <span class="open">
                <button class="plain" onclick={() => recoveryShown && copy(recoveryShown)}>Copy</button>
              </span>
              {#if enc?.configured && !encResetting}
                <button onclick={() => ((recoveryShown = null), (recoverySaved = false))}>Done</button>
              {:else}
                <label class="check">
                  <input type="checkbox" bind:checked={recoverySaved} /> I saved the recovery key
                </label>
                <button disabled={!recoverySaved} onclick={encConfirm} data-testid="enc-confirm"
                  >{encResetting ? 'Replace the password' : 'Turn on encryption'}</button
                >
              {/if}
            </div>
          {/if}
          {#if encForm === 'setup'}
            <form
              class="encform"
              onsubmit={(e) => {
                e.preventDefault()
                if (encPasswordOk()) void encSetup()
              }}
            >
              <label
                >Password (8 to 200 characters)
                <input type="password" autocomplete="new-password" bind:value={encPw} /></label
              >
              <label
                >Password again
                <input type="password" autocomplete="new-password" bind:value={encPw2} /></label
              >
              <span class="open">
                <button type="submit" disabled={!encPasswordOk()}>Create</button>
                <button type="button" class="plain" onclick={closeEncForm}>Cancel</button>
              </span>
            </form>
          {:else if encForm === 'unlock'}
            <form
              class="encform"
              onsubmit={(e) => {
                e.preventDefault()
                void encUnlock()
              }}
            >
              <label
                >Password or recovery key
                <input type="password" autocomplete="current-password" bind:value={encPw} /></label
              >
              <span class="open">
                <button type="submit" disabled={!encPw}>Unlock</button>
                <button type="button" class="plain" onclick={closeEncForm}>Cancel</button>
              </span>
            </form>
          {:else if encForm === 'password'}
            <form
              class="encform"
              onsubmit={(e) => {
                e.preventDefault()
                if (encPw.length >= 8 && encPw === encPw2) void encChange()
              }}
            >
              <label
                >Current password
                <input type="password" autocomplete="current-password" bind:value={encOld} /></label
              >
              <label
                >New password (8 to 200 characters)
                <input type="password" autocomplete="new-password" bind:value={encPw} /></label
              >
              <label
                >New password again
                <input type="password" autocomplete="new-password" bind:value={encPw2} /></label
              >
              <span class="open">
                <button type="submit" disabled={!encOld || !encPasswordOk()}>Change password</button>
                <button type="button" class="plain" onclick={closeEncForm}>Cancel</button>
              </span>
            </form>
          {:else if encForm === 'recovery'}
            <form
              class="encform"
              onsubmit={(e) => {
                e.preventDefault()
                void encNewRecovery()
              }}
            >
              <label
                >Current password
                <input type="password" autocomplete="current-password" bind:value={encOld} /></label
              >
              <p class="muted">
                The old recovery key stops working for new backups. Backups made before keep it.
              </p>
              <span class="open">
                <button type="submit" disabled={!encOld}>Make a new recovery key</button>
                <button type="button" class="plain" onclick={closeEncForm}>Cancel</button>
              </span>
            </form>
          {:else if encForm === 'reset'}
            <form
              class="encform"
              data-testid="enc-reset-form"
              onsubmit={(e) => {
                e.preventDefault()
                if (encUnderstood && encPasswordOk()) void encReset()
              }}
            >
              <p class="warn">
                Use this only if the password and the recovery key are both lost. Backups that are already
                sealed stay unreadable for good. New backups are sealed with the new password.
              </p>
              <label class="check">
                <input type="checkbox" bind:checked={encUnderstood} data-testid="enc-reset-understood" /> I understand
                that the encrypted backups made so far are lost
              </label>
              <label
                >New password (8 to 200 characters)
                <input type="password" autocomplete="new-password" bind:value={encPw} /></label
              >
              <label
                >New password again
                <input type="password" autocomplete="new-password" bind:value={encPw2} /></label
              >
              <span class="open">
                <button type="submit" disabled={!encUnderstood || !encPasswordOk()}>Reset the password</button
                >
                <button type="button" class="plain" onclick={closeEncForm}>Cancel</button>
              </span>
            </form>
          {/if}
          {#if enc?.configured}
            <p class="encmore">
              <button class="plain" onclick={() => (encForm = 'password')}>Change password…</button>
              <button class="plain" onclick={() => (encForm = 'recovery')}>New recovery key…</button>
              <button
                class="plain"
                data-testid="enc-forgot"
                onclick={() => (encForm = encForm === 'reset' ? '' : 'reset')}>Forgot the password…</button
              >
              {#if enc.plain_snapshots > 0}
                <button class="plain" onclick={encExisting} data-testid="enc-existing"
                  >Encrypt {enc.plain_snapshots} existing backup{enc.plain_snapshots === 1 ? '' : 's'}</button
                >
              {/if}
            </p>
          {/if}
          <SettingRow
            title="Second folder"
            description="Optional. Every snapshot is copied there too, for example to a USB disk or a network share. The folder must exist; while it is missing (disk not plugged in) the copy waits and catches up later."
            labelFor="backup-mirror"
          >
            <span class="open"
              ><input
                id="backup-mirror"
                bind:value={bkMirror}
                placeholder="none"
                onchange={() => setMirrorDir(bkMirror)}
              />
              {#if backend.pickFolder}
                <button
                  type="button"
                  data-testid="browse-mirror"
                  onclick={async () => {
                    const picked = await backend.pickFolder?.()
                    if (picked) await setMirrorDir(picked)
                  }}>Choose a folder…</button
                >
              {/if}
              <button class="plain" disabled={!s.backup_mirror_dir} onclick={() => setMirrorDir('')}
                >Remove</button
              ></span
            >
          </SettingRow>
          {#if bk?.mirror_error}
            <p class="muted" role="alert" data-testid="mirror-error">
              The second folder is not up to date: {bk.mirror_error}
            </p>
          {/if}
          <SettingRow
            title="Back up now"
            description={`${
              bk?.backups.length
                ? `Last: ${bk.backups[0].created}.`
                : 'No backup of this graph in the folder yet.'
            } Also when the app closes, if something changed in the last hour.`}
          >
            <button disabled={bkBusy} onclick={backupNow} data-testid="backup-now">Back up now</button>
          </SettingRow>
          {#if bk?.error}
            <p class="muted" role="alert" data-testid="backup-error">
              The last automatic backup failed: {bk.error}
            </p>
          {/if}
          <h3>How to restore</h3>
          <ol class="howto">
            <li>
              <strong>One page:</strong> choose a snapshot below, <em>Restore…</em>, type the page name and
              press
              <em>Restore page</em>. That page's file is replaced by its version in the snapshot. Before that,
              the page as it is now is saved in a new snapshot, so you can go back.
            </li>
            <li>
              <strong>Everything:</strong> <em>Restore…</em>, type a new or empty folder (an absolute path)
              and press <em>Restore folder</em>. The snapshot is unpacked there. Then open that folder as a
              graph (Settings → Graph → <em>Open or create another graph</em>) to look around or copy files
              back, or use the folder as your graph from now on. The open graph itself is never replaced as a
              whole.
            </li>
            <li>
              <strong>Device settings:</strong> <em>Restore…</em>, <em>Compare…</em> lists the lines that
              differ (<span class="add">+</span> in the snapshot, <span class="del">−</span> now).
              <em>Restore settings</em>
              replaces <code>~/.config/notatnik/settings.toml</code>; the file it replaces is kept next to it
              as
              <code>settings.toml.before-restore-…</code>. No page changes. The settings are in the snapshot
              under
              <code>.notatnik-device/</code> together with the window state of the graph; <em>Everything</em> does
              not unpack them into the graph folder.
            </li>
            <li>
              <strong>Without the app:</strong> a snapshot is a normal zip file. Unzip it with any tool; the result
              is a graph folder of Markdown files.
            </li>
            <li>
              Snapshots marked <em>before a change</em> were taken automatically before a rename or delete of
              several pages. For a single page's earlier states use <em>History</em> under the page's title.
            </li>
          </ol>
          <h3>Snapshots</h3>
          <ul class="snaps" data-testid="backup-list" aria-label="Snapshots">
            {#each bk?.backups ?? [] as b (b.name)}
              <li data-testid="backup-item">
                <span class="when">{b.created}</span>
                <span class="muted">{mb(b.bytes)}</span>
                {#if b.encrypted}<span class="badge" title="Sealed with the backup password"
                    >🔒 encrypted</span
                  >{/if}
                {#if bk?.mirror_dir}<span
                    class="badge"
                    title={b.in_mirror ? 'Also in the second folder' : 'Not yet copied to the second folder'}
                    >{b.in_mirror ? 'copied' : 'not copied'}</span
                  >{/if}
                {#if b.before}<span
                    class="badge"
                    title="Taken automatically before a rename, delete or restore">before a change</span
                  >{/if}
                <button class="plain" aria-expanded={restoreOf === b.name} onclick={() => openRestore(b.name)}
                  >Restore…</button
                >
                {#if restoreOf === b.name}
                  <div class="restore">
                    {#if b.encrypted && !enc?.unlocked}
                      <form
                        class="encform"
                        onsubmit={(e) => {
                          e.preventDefault()
                          void encUnlock()
                        }}
                      >
                        <label
                          >This backup is encrypted. Password or recovery key
                          <input
                            type="password"
                            autocomplete="current-password"
                            aria-label="Password or recovery key"
                            bind:value={encPw}
                          /></label
                        >
                        <span class="open"><button type="submit" disabled={!encPw}>Unlock</button></span>
                      </form>
                    {:else}
                      <div class="changes" data-testid="backup-changes">
                        <strong>Changes since this snapshot</strong>
                        {#if !changes}
                          <p class="muted">Comparing…</p>
                        {:else if !changes.length}
                          <p class="muted">Nothing differs: all files are the same as in this snapshot.</p>
                        {:else}
                          <p class="muted">
                            {changes.length} file{changes.length === 1 ? '' : 's'} differ.
                            <span class="add">+</span>
                            lines in the snapshot (a restore brings them back), <span class="del">−</span>
                            lines now.
                            <button class="plain" onclick={() => showAll(b.name, true)}
                              >Show all changes</button
                            >
                            <button class="plain" onclick={() => showAll(b.name, false)}>Hide all</button>
                          </p>
                          <ul class="chg">
                            {#each changes as c (c.path)}
                              <li>
                                <button
                                  class="plain row"
                                  aria-expanded={!!openPaths[c.path]}
                                  disabled={c.binary}
                                  onclick={() => showDiff(b.name, c.path, !openPaths[c.path])}
                                >
                                  <span class="badge">{CHANGE_LABEL[c.status]}</span>
                                  <code>{c.path}</code>
                                  {#if c.binary}<span class="muted">not text</span>{:else}<span class="muted"
                                      ><span class="add">+{c.added}</span>
                                      <span class="del">−{c.removed}</span></span
                                    >{/if}
                                </button>
                                {#if openPaths[c.path] && diffs[c.path]}
                                  {#if diffs[c.path].too_large}
                                    <p class="muted">Too long and too different to show line by line.</p>
                                  {:else}
                                    <pre
                                      class="diff"
                                      aria-label={`Changes in ${c.path}`}>{#each diffs[c.path].lines as l, i (i)}<span
                                          class:add={l.kind === 'add'}
                                          class:del={l.kind === 'del'}
                                          class:gap={l.kind === 'gap'}
                                          >{l.kind === 'add'
                                            ? '+ '
                                            : l.kind === 'del'
                                              ? '− '
                                              : l.kind === 'gap'
                                                ? '…'
                                                : '  '}{l.text}
</span>{/each}</pre>
                                  {/if}
                                {/if}
                              </li>
                            {/each}
                          </ul>
                        {/if}
                      </div>
                      <label
                        >One page, replacing the current one
                        <span class="open"
                          ><input aria-label="Page name" placeholder="page name" bind:value={restorePage} />
                          <button disabled={!restorePage.trim()} onclick={() => restoreOnePage(b.name)}
                            >Restore page</button
                          ></span
                        ></label
                      >
                      <div>
                        Device settings of this device (theme, dates, graph list, backup options)
                        <span class="open">
                          <button onclick={() => compareDevice(b.name)}>Compare…</button>
                        </span>
                        {#if devicePreview}
                          {#if !devicePreview.available}
                            <p class="muted">This snapshot has no device settings.</p>
                          {:else if devicePreview.same}
                            <p class="muted">The same as now.</p>
                          {:else}
                            <pre
                              class="diff"
                              data-testid="device-diff"
                              aria-label="Changed settings">{#each devicePreview.lines as l, i (i)}<span
                                  class:add={l.kind === 'add'}
                                  class:del={l.kind === 'del'}
                                  >{l.kind === 'add' ? '+ ' : '− '}{l.text}
</span>{/each}</pre>
                            <button onclick={() => restoreDevice(b.name)}>Restore settings</button>
                          {/if}
                        {/if}
                      </div>
                      <label
                        >Everything, into a new folder
                        <span class="open"
                          ><input
                            aria-label="Target folder"
                            placeholder="/path/to/new-folder"
                            bind:value={restoreTarget}
                          />
                          <button disabled={!restoreTarget.trim()} onclick={() => restoreFolder(b.name)}
                            >Restore folder</button
                          ></span
                        ></label
                      >
                    {/if}
                  </div>
                {/if}
              </li>
            {:else}
              <li class="muted">None yet.</li>
            {/each}
          </ul>
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
          <UnusedImages />
          <SettingRow
            title="Your graphs"
            description="The graphs you opened on this device, with a way to remove one from the list. Removing only changes the list: the folder, its notes and its backups stay."
          >
            <button class="plain" data-testid="switch-graph" onclick={() => (app.needsGraph = true)}
              >Switch graph…</button
            >
          </SettingRow>
          <SettingRow
            title="Open the last graph at start"
            description="This device. Off: the start screen lists your graphs with the last one selected (Enter opens it). On: the last graph opens without asking."
            labelFor="open-last-graph"
          >
            <input
              id="open-last-graph"
              class="switch"
              type="checkbox"
              role="switch"
              checked={s.open_last_graph}
              onchange={(e) => save({ open_last_graph: (e.currentTarget as HTMLInputElement).checked })}
            />
          </SettingRow>
          <SettingRow
            title="Open or create another graph"
            description="Enter the path of a folder. If it has no graph yet, one is created there."
            labelFor="graph-path"
          >
            <span class="open"
              ><input id="graph-path" bind:value={path} placeholder="/path/to/folder" />
              <button disabled={!path.trim()} onclick={() => openGraph(path.trim(), true)}>Open</button>
              {#if backend.pickFolder}
                <button
                  type="button"
                  data-testid="browse-folder"
                  onclick={async () => {
                    const picked = await backend.pickFolder?.()
                    if (picked) await openGraph(picked, true)
                  }}>Choose a folder…</button
                >
              {/if}</span
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
  .howto {
    margin: 0 0 1em;
    padding-left: 1.3em;
    max-width: 42em;
    font-size: 0.9em;
  }
  .howto li {
    margin: 0.3em 0;
  }
  .snaps {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .snaps li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.3em 1em;
    padding: 0.4em 0;
    border-bottom: 1px solid var(--border);
  }
  .snaps .when {
    font-variant-numeric: tabular-nums;
  }
  .restore {
    flex-basis: 100%;
    display: grid;
    gap: 0.6em;
    padding: 0.3em 0 0.5em;
  }
  .restore label,
  .restore > div {
    display: grid;
    gap: 0.2em;
    font-size: 0.9em;
  }
  .diff {
    margin: 0.2em 0;
    padding: 0.4em 0.6em;
    overflow-x: auto;
    font-size: 0.85em;
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .diff span {
    display: block;
  }
  .gap {
    color: var(--muted);
  }
  .recovery {
    display: grid;
    gap: 0.6em;
    justify-items: start;
    margin: 0.5em 0 1em;
    padding: 0.8em 1em;
    border: 1px solid var(--warn, var(--border));
    border-radius: 6px;
    max-width: 42em;
  }
  .recovery p {
    margin: 0;
  }
  .key {
    font-size: 1.05em;
    letter-spacing: 0.05em;
    word-break: break-word;
    user-select: all;
  }
  .check {
    display: flex;
    gap: 0.5em;
    align-items: center;
  }
  .encform .warn {
    color: var(--danger);
    font-size: 0.9em;
    margin: 0;
  }
  .encform {
    display: grid;
    gap: 0.5em;
    justify-items: start;
    margin: 0.4em 0 1em;
    max-width: 24em;
  }
  .encform label {
    display: grid;
    gap: 0.2em;
    font-size: 0.9em;
    width: 100%;
  }
  .encmore {
    display: flex;
    flex-wrap: wrap;
    gap: 0.2em 1em;
    margin: 0 0 0.6em;
  }
  .chg {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 28em;
    overflow-y: auto;
  }
  .chg li {
    display: block;
    border: none;
    padding: 0;
  }
  .chg .row {
    display: flex;
    gap: 0.6em;
    align-items: baseline;
    text-align: left;
    width: 100%;
  }
  .add {
    color: var(--ok, #1a7f37);
  }
  .del {
    color: var(--danger, #cf222e);
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
