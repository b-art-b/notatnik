<script lang="ts">
  import { backend } from '../api'
  import { getDoc } from '../stores/pages.svelte'
  import { bus } from '../stores/events.svelte'
  import { startEditing, stopEditing } from '../stores/editing.svelte'
  import { getBlock } from '../tree'
  import { pageHash, propertyHash, segmentHash } from '../route'
  import BlockList from './BlockList.svelte'
  import Backlinks from './Backlinks.svelte'
  import PageTasks from './PageTasks.svelte'
  import Hierarchy from './Hierarchy.svelte'
  import Editor from './Editor.svelte'
  import RenameDialog from './RenameDialog.svelte'
  import DeleteDialog from './DeleteDialog.svelte'
  import HistoryDialog from './HistoryDialog.svelte'
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
  let renaming = $state(false)
  let deleting = $state(false)
  let history = $state(false)
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
                <span class="k">aliases</span><span>{view.frontmatter.aliases.join(', ')}</span>
              </div>{/if}
            {#if view.frontmatter?.tags.length}<div class="prop">
                <span class="k">tags</span>
                <span
                  >{#each view.frontmatter.tags as t (t)}<a class="tag" href={pageHash(t)}>#{t}</a>
                  {/each}</span
                >
              </div>{/if}
            {#if view.is_journal && !view.frontmatter?.props.some((p) => p.key === 'type')}
              <div class="prop" data-testid="default-type">
                <span class="k">type</span><span class="muted"
                  >journal (default; set a type here to change it)</span
                >
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
                <ValueInput
                  label={p.key}
                  value={displayValue(p.raw)}
                  loadValues={() => valuesOfKey(p.key)}
                  oncommit={(v) => setProp(p.key, v)}
                />
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
                list="page-prop-keys"
                bind:value={newKey}
                placeholder="property, e.g. type"
                aria-label="New property name"
              />
              <ValueInput
                bind:value={newValue}
                values={knownValues}
                placeholder="value, or a page"
                label="New property value"
              />
              <button type="submit" disabled={!validKey || !newValue.trim()}>Add</button>
              <datalist id="page-prop-keys"
                >{#each knownKeys as k (k)}<option value={k}></option>{/each}</datalist
              >
            </form>
          </div>
        {/if}
        {#if view.diagnostics.length}
          <ul class="diagnostics">
            {#each view.diagnostics as d, i (i)}<li class={d.severity}>{d.message}</li>{/each}
          </ul>
        {/if}
      </header>
    {/if}
    {#if renaming}<RenameDialog old={view.name} onclose={() => (renaming = false)} />{/if}
    {#if history}<HistoryDialog page={view.name} onclose={() => (history = false)} />{/if}
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
    {#if showBacklinks && !compact}<Hierarchy page={view.name} /><PageTasks page={view.name} /><Backlinks
        page={view.name}
      />{/if}
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
  .fm {
    margin: 0.3em 0 0.8em;
    display: flex;
    flex-direction: column;
    gap: 0.25em;
    font-size: 0.9em;
  }
  .prop {
    display: flex;
    align-items: center;
    gap: 0.5em;
  }
  .prop .k {
    min-width: 6em;
    color: var(--muted);
  }
  .prop input {
    flex: 1;
    min-width: 0;
    max-width: 22em;
  }
  a.k {
    color: var(--muted);
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
    font-size: 0.85em;
    white-space: nowrap;
  }
  .prop .x {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .prop.newprop {
    margin-top: 0.3em;
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
  .add {
    opacity: 0;
  }
  .blocks:hover .add {
    opacity: 1;
  }
  .draft {
    padding-left: 36px;
  }
  .error {
    color: var(--danger);
  }
</style>
