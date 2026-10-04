<script lang="ts">
  import { getDoc } from '../stores/pages.svelte'
  import { startEditing, stopEditing } from '../stores/editing.svelte'
  import { getBlock } from '../tree'
  import { pageHash } from '../route'
  import BlockList from './BlockList.svelte'
  import Backlinks from './Backlinks.svelte'
  import Editor from './Editor.svelte'
  import RenameDialog from './RenameDialog.svelte'
  import { formatDate } from '../dates'

  interface Props {
    name: string
    viewKey: string
    zoom?: number[]
    showBacklinks?: boolean
    compact?: boolean
  }
  let { name, viewKey, zoom = [], showBacklinks = true, compact = false }: Props = $props()

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
  let showProps = $state(false)

  async function commitDraft(text: string) {
    if (text.trim() === '') return
    draft = false
    const v = await doc.apply([{ op: 'insert', parent: [], index: 0, text }])
    const b = v?.blocks[0]
    if (b) startEditing(viewKey, doc.name, b.id, 'end')
  }
  const isoTitle = $derived(view?.is_journal ? formatDate(view.name) : null)
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
              <a href={pageHash(segments.slice(0, i + 1).join('/'))}>{seg}</a><span class="sep">/</span>
            {/if}
          {/each}
        </nav>
        <h1>
          <button class="title" title="Rename page" onclick={() => (renaming = !view.is_journal)}
            >{isoTitle ?? view.frontmatter?.title ?? segments[segments.length - 1]}</button
          >
          {#if isoTitle}<span class="muted iso">{view.name}</span>{/if}
        </h1>
        {#if crumbs.length || zoom.length}
          <nav class="breadcrumb zoom" aria-label="Block path">
            <a href={pageHash(view.name)}>{view.name}</a>
            {#each crumbs as c (c.path.join('.'))}<span class="sep">›</span><a
                href={pageHash(view.name, c.path)}>{c.text || '…'}</a
              >{/each}
          </nav>
        {/if}
        {#if view.frontmatter && (view.frontmatter.tags.length || view.frontmatter.aliases.length || view.frontmatter.props.length)}
          <button class="props-toggle" onclick={() => (showProps = !showProps)} aria-expanded={showProps}
            >{showProps ? '▾' : '▸'} properties</button
          >
          {#if showProps}
            <dl class="fm">
              {#if view.frontmatter.aliases.length}<dt>aliases</dt>
                <dd>{view.frontmatter.aliases.join(', ')}</dd>{/if}
              {#if view.frontmatter.tags.length}<dt>tags</dt>
                <dd>
                  {#each view.frontmatter.tags as t (t)}<a class="tag" href={pageHash(t)}>#{t}</a>
                  {/each}
                </dd>{/if}
              {#each view.frontmatter.props as p (p.key)}<dt>{p.key}</dt>
                <dd>{p.raw}</dd>{/each}
            </dl>
          {/if}
        {/if}
        {#if view.diagnostics.length}
          <ul class="diagnostics">
            {#each view.diagnostics as d, i (i)}<li class={d.severity}>{d.message}</li>{/each}
          </ul>
        {/if}
      </header>
      {#if view.child_pages.length}
        <section class="children-pages" aria-label="Child pages">
          <h2>Pages in this namespace</h2>
          <ul>
            {#each view.child_pages as c (c)}<li><a href={pageHash(c)}>{c}</a></li>{/each}
          </ul>
        </section>
      {/if}
    {/if}
    {#if renaming}<RenameDialog old={view.name} onclose={() => (renaming = false)} />{/if}
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
    {#if showBacklinks && !compact}<Backlinks page={view.name} />{/if}
  </article>
{/if}

<style>
  .page {
    max-width: 880px;
  }
  h1 {
    margin: 0.1em 0 0.4em;
    font-size: 1.7em;
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
  .breadcrumb a {
    color: var(--muted);
  }
  .sep {
    margin: 0 0.3em;
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
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.1em 1em;
    font-size: 0.85em;
    margin: 0.3em 0;
  }
  .fm dt {
    color: var(--muted);
  }
  .fm dd {
    margin: 0;
  }
  .diagnostics {
    font-size: 0.8em;
    color: var(--warn);
  }
  .children-pages h2 {
    font-size: 0.9em;
    color: var(--muted);
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
