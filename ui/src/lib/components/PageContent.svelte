<script lang="ts">
  import { setContext, untrack } from 'svelte'
  import { backend, CommandError } from '../api'
  import { app, onSide, toast, errorMessage } from '../stores/app.svelte'
  import { getDoc } from '../stores/pages.svelte'
  import { bus } from '../stores/events.svelte'
  import { startEditing, stopEditing } from '../stores/editing.svelte'
  import { getBlock } from '../tree'
  import { pageHash, propertyHash, segmentHash } from '../route'
  import BlockList from './BlockList.svelte'
  import Backlinks from './Backlinks.svelte'
  import PageTasks from './PageTasks.svelte'
  import DayEvents from './DayEvents.svelte'
  import Hierarchy from './Hierarchy.svelte'
  import Editor from './Editor.svelte'
  import RenameDialog from './RenameDialog.svelte'
  import DeleteDialog from './DeleteDialog.svelte'
  import HistoryDialog from './HistoryDialog.svelte'
  import EncryptDialog from './EncryptDialog.svelte'
  import { encryption, pathIsEncrypted, refreshEncryption } from '../stores/encryption.svelte'
  import ValueInput from './ValueInput.svelte'
  import { displayValue, linkedPages, yamlValue } from '../props'
  import { isArchived, isArchivedItself } from '../stores/archived.svelte'
  import { shownDate } from '../stores/format.svelte'

  interface Props {
    name: string
    viewKey: string
    zoom?: number[]
    showBacklinks?: boolean
    compact?: boolean
    /** A journal's date title opens that day as its own page (used in the journals feed). */
    titleLink?: boolean
    /** No "properties" line: the journals landing page is for writing, a day's properties are on its own page. */
    hideProps?: boolean
  }
  let {
    name,
    viewKey,
    zoom = [],
    showBacklinks = true,
    compact = false,
    titleLink = false,
    hideProps = false,
  }: Props = $props()

  const doc = $derived(getDoc(name))
  const view = $derived(doc.view)
  // query blocks below read the page they sit on from here (`{{page}}` in a query means this page); a getter, because
  // `name` can change while the component lives
  setContext('queryPage', () => name)
  const roots = $derived.by(() => {
    if (!view) return []
    if (zoom.length) {
      const b = getBlock(view.blocks, zoom)
      return b ? [b] : view.blocks
    }
    return view.blocks
  })
  const crumbs = $derived.by(() => {
    if (!view || !zoom.length) return []
    return zoom.slice(0, -1).map((_, i) => {
      const b = getBlock(view.blocks, zoom.slice(0, i + 1))
      return { text: (b?.text ?? '').split('\n')[0], path: zoom.slice(0, i + 1) }
    })
  })
  const segments = $derived((view?.name ?? name).split('/'))

  // A segment of this page's name that occurs in several places is a topic (`Renewal` in P/Capgemini/Renewal and
  // P/Infomotion/Renewal): it links to the view of all its places, in the path above the title and next to the title.
  let segPlaces = $state<number[]>([])
  $effect(() => {
    void bus.version
    const names = view && !view.is_journal && view.name.includes('/') ? view.name.split('/') : []
    let cancelled = false
    if (names.length === 0) {
      segPlaces = []
      return
    }
    backend.call('segment_places', { segments: names }).then((r) => {
      if (!cancelled) segPlaces = r.places
    })
    return () => {
      cancelled = true
    }
  })
  // The template of this page's `type` and what of it the page lacks: offered as a button below the properties.
  let tpl = $state<{ template: string | null; missing: string[] } | null>(null)
  $effect(() => {
    void bus.version
    void view?.hash
    const n = name
    let cancelled = false
    backend.call('template_info', { page: n }).then((r) => {
      if (!cancelled) tpl = r
    })
    return () => {
      cancelled = true
    }
  })
  async function applyTemplate() {
    try {
      const r = await backend.call('apply_template', { page: name })
      toast(`Added ${r.template_added.join(', ')} from the template “${r.template}”.`)
      await doc.load()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  // "Save as template": this page's `type`, properties and blocks become `templates/<type>.md`
  let savingTpl = $state(false)
  let keepValues = $state(false)
  let tplExists = $state(false)
  const hasType = $derived(!!view?.frontmatter?.props.some((p) => p.key.toLowerCase() === 'type'))
  async function saveTemplate(replace: boolean) {
    try {
      const r = await backend.call('save_template', { page: name, keep_values: keepValues, replace })
      toast(`Saved the template “${r.name}” (${r.fields.length} fields, ${r.blocks} blocks).`)
      savingTpl = false
      tplExists = false
    } catch (e) {
      if (e instanceof CommandError && e.code === 'exists') tplExists = true
      else toast(errorMessage(e), 'error')
    }
  }
  const lastPlaces = $derived(segPlaces[segments.length - 1] ?? 0)
  // Progressive rendering (NF-4): the first blocks appear at once, the rest stream in over the next frames.
  const FIRST_BATCH = 120
  const BATCH = 240
  let limit = $state(FIRST_BATCH)
  $effect(() => {
    void name
    limit = FIRST_BATCH
  })
  $effect(() => {
    const total = roots.length
    if (limit >= total) return
    const id = requestAnimationFrame(() => (limit = Math.min(total, limit + BATCH)))
    return () => cancelAnimationFrame(id)
  })
  $effect(() => {
    const all = () => (limit = Number.MAX_SAFE_INTEGER)
    window.addEventListener('notatnik-flash', all)
    window.addEventListener('notatnik-render-all', all)
    return () => {
      window.removeEventListener('notatnik-flash', all)
      window.removeEventListener('notatnik-render-all', all)
    }
  })
  let draft = $state(false)
  // a page just made from "New page": its first block opens for typing (only once the page is loaded and empty)
  $effect(() => {
    const want = app.draftFor
    const v = view
    if (!want || !v || v.name.toLowerCase() !== want.toLowerCase()) return
    untrack(() => {
      app.draftFor = ''
      if (v.blocks.length === 0 && !v.preamble) {
        draft = true
        stopEditing()
      }
    })
  })
  let renaming = $state(false)
  let deleting = $state(false)
  let history = $state(false)
  let encrypting = $state(false)
  // the page is stored encrypted on the server: one of the marked paths covers its file (EN-12)
  const encrypted = $derived(
    !!encryption.status && !!view?.path && pathIsEncrypted(encryption.status.paths, view.path),
  )
  $effect(() => {
    // read once per page view; the dialog and Settings keep the store fresh after a change
    if (!encryption.status && app.graph?.remote) void refreshEncryption()
  })
  let showProps = $state(false)

  // --- page properties (frontmatter): edit, remove, add ---
  let newKey = $state('')
  let newValue = $state('')
  let knownKeys = $state<string[]>([])
  let knownValues = $state<string[]>([])
  const propCount = $derived(
    (view?.frontmatter?.props.length ?? 0) +
      (view?.frontmatter?.tags.length ? 1 : 0) +
      (view?.frontmatter?.aliases.length ? 1 : 0),
  )
  const validKey = $derived(/^[A-Za-z0-9_][A-Za-z0-9_.-]*$/.test(newKey.trim()))

  async function openProps() {
    showProps = !showProps
    if (showProps) knownKeys = (await backend.call('page_property_keys', {})).map((k) => k.key)
  }
  // values already used for the typed key (the datalist offers them)
  $effect(() => {
    const k = newKey.trim()
    if (!showProps || !knownKeys.includes(k)) {
      knownValues = []
      return
    }
    let cancelled = false
    backend.call('page_groups', { key: k }).then((g) => {
      if (!cancelled) knownValues = g.flatMap((x) => (x.value === null ? [] : [x.value]))
    })
    return () => {
      cancelled = true
    }
  })

  /** The separate values of a raw YAML value (`[red, blue]` → red, blue), for the "pages with this value" links. */
  function valueItems(raw: string): string[] {
    const t = raw.trim()
    const inner = /^\[(?!\[).*\]$/.test(t) ? t.slice(1, -1).split(',') : [t]
    return inner.map((x) => x.trim().replace(/^(["'])(.*)\1$/, '$2')).filter((x) => x && !x.includes('[['))
  }

  /** Values in use for a property of other pages (offered when its value field is focused). */
  async function valuesOfKey(key: string): Promise<string[]> {
    const g = await backend.call('page_groups', { key })
    return g.flatMap((x) => (x.value === null ? [] : [x.value]))
  }
  async function setProp(key: string, value: string) {
    await doc.apply([{ op: 'set_page_property', key, value: yamlValue(value) }])
  }
  async function toggleArchive() {
    if (isArchivedItself(view?.name ?? '')) await removeProp('archived')
    else await doc.apply([{ op: 'set_page_property', key: 'archived', value: 'true' }])
  }
  async function removeProp(key: string) {
    await doc.apply([{ op: 'remove_page_property', key }])
  }
  async function addProp(e: SubmitEvent) {
    e.preventDefault()
    if (!validKey || !newValue.trim()) return
    await doc.apply([{ op: 'set_page_property', key: newKey.trim(), value: yamlValue(newValue) }])
    newKey = ''
    newValue = ''
    knownKeys = (await backend.call('page_property_keys', {})).map((k) => k.key)
  }

  async function commitDraft(text: string) {
    if (text.trim() === '') return
    draft = false
    const v = await doc.apply([{ op: 'insert', parent: [], index: 0, text }])
    const b = v?.blocks[0]
    if (b) startEditing(viewKey, doc.name, b.id, 'end')
  }
  const isoTitle = $derived(view?.is_journal ? shownDate(view.name) : null)
  import PageIcon from './PageIcon.svelte'
</script>

{#if doc.loadError}
  <p class="error">{doc.loadError}</p>
{:else if !view}
  <p class="muted">Loading…</p>
{:else}
  <article class="page" data-page={view.name} aria-label={view.title}>
    {#if !compact}
      <header>
        <nav class="breadcrumb" aria-label="Namespace">
          {#each segments as seg, i (i)}
            {#if i < segments.length - 1}
              {#if (segPlaces[i] ?? 0) > 1}<a
                  class="topic"
                  href={segmentHash(seg)}
                  data-segment={seg}
                  title={`“${seg}” occurs in ${segPlaces[i]} places: show them all`}
                  >{seg}<sup class="badge">{segPlaces[i]}</sup></a
                >{:else}<a href={pageHash(segments.slice(0, i + 1).join('/'))}>{seg}</a>{/if}<span class="sep"
                >/</span
              >
            {/if}
          {/each}
        </nav>
        <h1>
          {#if titleLink && isoTitle}
            <a class="title" href={pageHash(view.name)} title="Open this day as a page"
              ><PageIcon name={view.name} journal />{isoTitle}</a
            >
          {:else}
            <button class="title" title="Rename page" onclick={() => (renaming = !view.is_journal)}
              ><PageIcon name={view.name} />{isoTitle ??
                view.frontmatter?.title ??
                segments[segments.length - 1]}</button
            >
          {/if}
          {#if encrypted}<span
              class="lock"
              data-testid="title-lock"
              role="img"
              aria-label="Stored encrypted on the server"
              title="Stored encrypted on the server">🔒</span
            >{/if}
          {#if isoTitle && !titleLink}<span class="muted iso">{view.name}</span>{/if}
          {#if lastPlaces > 1}<a
              class="topic-chip"
              href={segmentHash(segments[segments.length - 1])}
              data-testid="title-topic"
              title={`“${segments[segments.length - 1]}” occurs in ${lastPlaces} places: show them all`}
              >in {lastPlaces} places</a
            >{/if}
        </h1>
        {#if crumbs.length || zoom.length}
          <nav class="breadcrumb zoom" aria-label="Block path">
            <a href={pageHash(view.name)}>{view.name}</a>
            {#each crumbs as c (c.path.join('.'))}<span class="sep">›</span><a
                href={pageHash(view.name, c.path)}>{c.text || '…'}</a
              >{/each}
          </nav>
        {/if}
        {#if !hideProps}
          <span class="page-actions">
            <button
              class="act"
              data-testid="history-toggle"
              title="Earlier versions of this page"
              aria-expanded={history}
              onclick={() => (history = !history)}>History</button
            >
            {#if app.graph?.remote}
              <button
                class="act"
                data-testid="encrypt-toggle"
                title="Store this page encrypted on the server"
                aria-expanded={encrypting}
                onclick={() => (encrypting = !encrypting)}>{encrypted ? 'Encrypted' : 'Encrypt…'}</button
              >
            {/if}
            {#if !view.is_journal}
              <button
                class="act"
                data-testid="archive-toggle"
                title={isArchivedItself(view.name)
                  ? 'Show this page in the sidebar and the agenda again'
                  : 'Hide this page (and the pages below it) from the sidebar and the agenda'}
                onclick={toggleArchive}>{isArchivedItself(view.name) ? 'Unarchive' : 'Archive'}</button
              >
              <button class="act danger" data-testid="delete-page" onclick={() => (deleting = !deleting)}
                >Delete…</button
              >
            {/if}
          </span>
        {/if}
        {#if !hideProps}
          <button
            class="props-toggle"
            onclick={openProps}
            aria-expanded={showProps}
            data-testid="props-toggle"
            >{showProps ? '▾' : '▸'} properties{propCount ? ` (${propCount})` : ''}</button
          >
        {/if}
        {#if showProps && !hideProps}
          <div class="fm" data-testid="page-props">
            {#if view.frontmatter?.aliases.length}<div class="prop">
                <span class="k">aliases</span><span class="v">{view.frontmatter.aliases.join(', ')}</span
                ><span></span>
              </div>{/if}
            {#if view.frontmatter?.tags.length}<div class="prop">
                <span class="k">tags</span>
                <span class="v"
                  >{#each view.frontmatter.tags as t (t)}<a class="tag" href={pageHash(t)}>#{t}</a>
                  {/each}</span
                ><span></span>
              </div>{/if}
            {#if view.is_journal && !view.frontmatter?.props.some((p) => p.key === 'type')}
              <div class="prop" data-testid="default-type">
                <span class="k">type</span><span class="v muted"
                  >journal (default; set a type here to change it)</span
                ><span></span>
              </div>
            {/if}
            {#each view.frontmatter?.props ?? [] as p (p.key)}
              <div class="prop">
                <a
                  class="k"
                  href={propertyHash(p.key)}
                  title={`All pages with “${p.key}”`}
                  data-testid="prop-key-link">{p.key}</a
                >
                <div class="v">
                  <ValueInput
                    block
                    label={p.key}
                    value={displayValue(p.raw)}
                    loadValues={() => valuesOfKey(p.key)}
                    oncommit={(v) => setProp(p.key, v)}
                  />
                  {#if linkedPages(displayValue(p.raw)).length || valueItems(p.raw).length}
                    <div class="chips">
                      {#each linkedPages(displayValue(p.raw)) as n (n)}
                        <a
                          class="pv"
                          href={pageHash(n)}
                          title={`Open ${n}`}
                          data-page={n}
                          data-testid="prop-page-link">→ {n}</a
                        >
                      {/each}
                      {#each valueItems(p.raw) as v (v)}
                        <a
                          class="pv"
                          href={propertyHash(p.key, v)}
                          title={`All pages with ${p.key}: ${v}`}
                          data-testid="prop-value-link">↗ {v}</a
                        >
                      {/each}
                    </div>
                  {/if}
                </div>
                <button
                  class="x"
                  aria-label={`Remove ${p.key}`}
                  title="Remove property"
                  onclick={() => removeProp(p.key)}>✕</button
                >
              </div>
            {/each}
            <form class="prop newprop" onsubmit={addProp}>
              <input
                class="keyfield"
                list="page-prop-keys"
                bind:value={newKey}
                placeholder="property"
                aria-label="New property name"
              />
              <div class="v">
                <ValueInput
                  block
                  bind:value={newValue}
                  values={knownValues}
                  placeholder="value, or a page"
                  label="New property value"
                />
              </div>
              <button type="submit" class="add-prop" disabled={!validKey || !newValue.trim()}>Add</button>
              <datalist id="page-prop-keys"
                >{#each knownKeys as k (k)}<option value={k}></option>{/each}</datalist
              >
            </form>
            {#if hasType || (tpl?.template && tpl.missing.length)}
              <div class="fm-actions">
                {#if hasType}
                  {#if savingTpl}
                    <div class="save-tpl" data-testid="save-template">
                      <label><input type="radio" bind:group={keepValues} value={false} /> Fields empty</label>
                      <label
                        ><input type="radio" bind:group={keepValues} value={true} /> Keep these values as defaults</label
                      >
                      {#if tplExists}
                        <span role="alert">A template for this type exists.</span>
                        <button class="act" onclick={() => saveTemplate(true)}>Replace it</button>
                      {:else}
                        <button class="act" onclick={() => saveTemplate(false)}>Save</button>
                      {/if}
                      <button
                        class="act"
                        onclick={() => {
                          savingTpl = false
                          tplExists = false
                        }}>Cancel</button
                      >
                    </div>
                  {:else}
                    <button class="act" data-testid="save-as-template" onclick={() => (savingTpl = true)}
                      >Save as template…</button
                    >
                  {/if}
                {/if}
                {#if tpl?.template && tpl.missing.length}
                  <button class="act" data-testid="apply-template" onclick={applyTemplate}
                    >Add missing from template “{tpl.template}” ({tpl.missing.join(', ')})</button
                  >
                {/if}
              </div>
            {/if}
          </div>
        {/if}
        {#if view.diagnostics.length}
          <ul class="diagnostics">
            {#each view.diagnostics as d, i (i)}<li class={d.severity}>{d.message}</li>{/each}
          </ul>
        {/if}
      </header>
    {/if}
    {#if view.is_journal && !compact}<DayEvents date={view.name} />{/if}
    {#if renaming}<RenameDialog old={view.name} onclose={() => (renaming = false)} />{/if}
    {#if history}<HistoryDialog page={view.name} onclose={() => (history = false)} />{/if}
    {#if encrypting}<EncryptDialog
        page={view.name}
        path={view.path}
        hasSubpages={view.child_pages.length > 0}
        onclose={() => (encrypting = false)}
      />{/if}
    {#if deleting}<DeleteDialog page={view.name} onclose={() => (deleting = false)} />{/if}
    {#if !compact && isArchived(view.name)}
      <p class="archived" data-testid="archived-banner" role="status">
        Archived: hidden from the sidebar and the agenda.
        {#if isArchivedItself(view.name)}
          <button class="act" onclick={toggleArchive}>Unarchive</button>
        {:else}
          It is below an archived page.
        {/if}
      </p>
    {/if}
    <div class="blocks">
      <div role={view.blocks.length || view.preamble ? 'tree' : undefined} aria-label="Blocks">
        {#if view.preamble}
          <BlockList blocks={[view.preamble]} docName={view.name} {viewKey} />
        {/if}
        <BlockList blocks={roots.slice(0, limit)} docName={view.name} {viewKey} />
      </div>
      {#if view.blocks.length === 0 && !view.preamble}
        {#if draft}
          <div class="draft">
            <span class="draft-dot" aria-hidden="true"></span>
            <Editor
              text=""
              cursor="start"
              pageName={view.name}
              onchange={commitDraft}
              onaction={async (a) => {
                if (a.kind === 'escape') draft = false
                return true
              }}
              onundo={() => {}}
              onpaste={async () => false}
              onblur={(t) => {
                if (t.trim() === '') draft = false
                else void commitDraft(t)
              }}
            />
          </div>
        {:else}
          <button class="empty" onclick={() => ((draft = true), stopEditing())}
            >Click here to start writing</button
          >
        {/if}
      {:else if !compact}
        <button
          class="add"
          aria-label="Add block"
          onclick={async () => {
            const v = await doc.apply([{ op: 'insert', parent: [], index: view.blocks.length, text: '' }])
            const b = v?.blocks[v.blocks.length - 1]
            if (b) startEditing(viewKey, doc.name, b.id, 'start')
          }}>+</button
        >
      {/if}
    </div>
    <!-- tasks and references go to the right panel when Settings → Appearance says so and the window is wide -->
    {#if showBacklinks && !compact}
      <Hierarchy
        page={view.name}
        lock={!!encryption.status &&
          !!view.path &&
          pathIsEncrypted(encryption.status.paths, view.path.replace(/\.md$/, '') + '/x.md')}
      />
      {#if !onSide('tasks')}<PageTasks page={view.name} />{/if}
      {#if !onSide('refs')}<Backlinks page={view.name} />{/if}
    {/if}
  </article>
{/if}

<style>
  .page {
    max-width: 880px;
  }
  h1 {
    margin: 0.1em 0 0.5em;
    font-size: 2.1em;
    font-weight: 700;
    letter-spacing: -0.022em;
    line-height: 1.2;
  }
  a.title {
    cursor: pointer;
    text-decoration: none;
    color: inherit;
  }
  a.title:hover {
    text-decoration: underline;
  }
  .title {
    font: inherit;
    border: none;
    background: none;
    color: inherit;
    cursor: text;
    padding: 0;
  }
  .iso {
    font-size: 0.5em;
    margin-left: 0.6em;
  }
  .breadcrumb {
    font-size: 0.85em;
    color: var(--muted);
  }
  .topic-chip {
    margin-left: 0.6em;
    font-size: 0.4em;
    font-weight: 500;
    vertical-align: middle;
    background: var(--chip);
    color: var(--muted);
    border-radius: 8px;
    padding: 0.1em 0.6em;
    text-decoration: none;
    white-space: nowrap;
  }
  .topic-chip:hover {
    color: var(--link);
  }
  .badge {
    margin-left: 0.15em;
    font-size: 0.65em;
  }
  .breadcrumb a {
    color: var(--muted);
  }
  .sep {
    margin: 0 0.3em;
  }
  .page-actions {
    display: inline-flex;
    gap: 0.4em;
    margin-right: 0.8em;
  }
  .save-tpl {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6em;
    color: var(--muted);
    font-size: 0.85em;
  }
  .lock {
    font-size: 0.6em;
    margin-left: 0.4em;
    vertical-align: middle;
  }
  .act {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0.1em 0.3em;
    border-radius: 4px;
    font-size: 0.85em;
  }
  .act:hover {
    background: var(--chip);
    color: var(--text);
  }
  .act.danger:hover {
    color: var(--danger);
  }
  .archived {
    background: var(--warn-bg);
    color: var(--warn);
    border-radius: 6px;
    padding: 0.3em 0.7em;
    font-size: 0.9em;
  }
  .props-toggle {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 0.8em;
    padding: 0;
  }
  /* the properties of a page: a card, key | value | remove in three aligned columns (each row is `display: contents`) */
  .fm {
    margin: 0.4em 0 0.9em;
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr) 1.8em;
    align-items: start;
    gap: 0.45em 0.8em;
    max-width: 38em;
    padding: 0.7em 0.9em;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 0.92em;
  }
  .prop {
    display: contents;
  }
  .prop .k {
    color: var(--muted);
    font-weight: 500;
    padding-top: 0.35em; /* the text lines up with the field's text */
  }
  .prop .v {
    min-width: 0;
    padding-top: 0.2em;
  }
  .prop .v.muted {
    color: var(--muted);
  }
  .prop input,
  .prop .v :global(input) {
    padding: 0.3em 0.5em;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--panel);
    color: var(--text);
    font: inherit;
  }
  .keyfield {
    width: 8.5em; /* also sets the width of the key column when the keys are short */
    box-sizing: border-box;
    margin-top: 0.2em;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.1em 0.8em;
    margin-top: 0.15em;
  }
  a.k {
    text-decoration: none;
  }
  a.k:hover,
  a.pv:hover {
    color: var(--link);
    text-decoration: underline;
  }
  a.pv {
    color: var(--muted);
    text-decoration: none;
    font-size: 0.82em;
    white-space: nowrap;
  }
  .prop .x {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    border-radius: 4px;
    padding: 0.3em 0;
    opacity: 0.7;
  }
  .prop .x:hover,
  .prop .x:focus-visible {
    opacity: 1;
    color: var(--danger);
    background: var(--chip);
  }
  .prop.newprop {
    padding-top: 0.5em;
  }
  .add-prop {
    grid-column: 1 / -1;
    justify-self: end;
  }
  .fm-actions {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: 0.2em 0.8em;
    padding-top: 0.5em;
    border-top: 1px solid var(--border);
  }
  .fm-actions .act {
    padding: 0.2em 0.5em;
  }

  .diagnostics {
    font-size: 0.8em;
    color: var(--warn);
  }
  .empty,
  .add {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0.4em 0;
  }
  .empty {
    /* the text starts where a block's text starts (bullet cell + gap) */
    padding-left: 20px;
  }
  .add {
    opacity: 0;
  }
  .blocks:hover .add {
    opacity: 1;
  }
  /* same geometry as a block row (Block.svelte): an 18px bullet cell, a 2px gap, then the text,
     so the text does not jump when the draft becomes the first real block */
  .draft {
    display: flex;
    align-items: flex-start;
    gap: 2px;
  }
  .draft-dot {
    flex: none;
    width: 18px;
    height: 1.6em;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .draft-dot::before {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted);
    opacity: 0.4;
  }
  .draft > :global(:not(.draft-dot)) {
    flex: 1;
    min-width: 0;
  }
  .error {
    color: var(--danger);
  }
</style>
