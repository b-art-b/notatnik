<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import './app.css'
  import { app, bootstrap } from './lib/stores/app.svelte'
  import { editing, isEditingDoc, stopEditing } from './lib/stores/editing.svelte'
  import { flushAll, getDoc, handleExternalChange, overallStatus } from './lib/stores/pages.svelte'
  import { bus } from './lib/stores/events.svelte'
  import {
    globalUndo,
    selectionDelete,
    selectionEdit,
    selectionIndent,
    selectionMarkdown,
    selectionMove,
    selectionOutdent,
    selectionStep,
  } from './lib/commands/exec'
  import { getBlock } from './lib/tree'
  import PageContent from './lib/components/PageContent.svelte'
  import JournalsView from './lib/components/JournalsView.svelte'
  import NamespaceTree from './lib/components/NamespaceTree.svelte'
  import SearchDialog from './lib/components/SearchDialog.svelte'
  import Palette from './lib/components/Palette.svelte'
  import Settings from './lib/components/Settings.svelte'
  import Welcome from './lib/components/Welcome.svelte'
  import ConflictBanner from './lib/components/ConflictBanner.svelte'
  import AgendaView from './lib/components/AgendaView.svelte'
  import SavedQueryPage from './lib/components/SavedQueryPage.svelte'
  import { backend } from './lib/api'
  import type { SavedQueryView } from './lib/api'
  import { pageHash } from './lib/route'

  const route = $derived(app.route)
  let saved = $state<SavedQueryView[]>([])
  $effect(() => {
    void app.savedVersion
    void app.graph
    if (app.graph) backend.call('list_saved_queries', {}).then((l) => (saved = l))
  })

  $effect(() => {
    void bus.version
    const e = bus.last
    // untracked: reloading documents reads their state, which must not re-trigger this effect
    untrack(() => {
      if (e && !e.own && e.pages.length) handleExternalChange(e.pages, isEditingDoc)
    })
  })

  onMount(() => {
    void bootstrap()
    const onKey = (e: KeyboardEvent) => void globalKey(e)
    window.addEventListener('keydown', onKey, true)
    const beforeUnload = () => void flushAll()
    window.addEventListener('beforeunload', beforeUnload)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('beforeunload', beforeUnload)
    }
  })

  async function globalKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey
    if (mod && e.key.toLowerCase() === 'k') {
      e.preventDefault()
      app.paletteOpen = !app.paletteOpen
      return
    }
    if (mod && e.shiftKey && e.key.toLowerCase() === 'f') {
      e.preventDefault()
      app.searchOpen = true
      return
    }
    if (app.paletteOpen || app.searchOpen) return
    const inField = (e.target as HTMLElement)?.closest('input, textarea, select, .cm-editor')
    if (editing.mode === 'select' && !inField) {
      const doc = getDoc(editing.docName)
      const ctx = { doc, viewKey: editing.viewKey }
      const k = e.key
      let handled = true
      if (k === 'ArrowUp' && e.altKey) void selectionMove(ctx, 'up')
      else if (k === 'ArrowDown' && e.altKey) void selectionMove(ctx, 'down')
      else if (k === 'ArrowUp') selectionStep(ctx, 'up', e.shiftKey)
      else if (k === 'ArrowDown') selectionStep(ctx, 'down', e.shiftKey)
      else if (k === 'Tab') void (e.shiftKey ? selectionOutdent(ctx) : selectionIndent(ctx))
      else if (k === 'Delete' || k === 'Backspace') void selectionDelete(ctx)
      else if (k === 'Enter') selectionEdit(ctx)
      else if (k === 'Escape') stopEditing()
      else if (mod && (k === 'c' || k === 'x')) {
        const md = selectionMarkdown(ctx)
        void navigator.clipboard.writeText(md)
        if (k === 'x') void selectionDelete(ctx)
      } else handled = false
      if (handled) e.preventDefault()
      if (handled) return
    }
    if (mod && !inField && e.key.toLowerCase() === 'z') {
      e.preventDefault()
      void globalUndo(e.shiftKey ? 'redo' : 'undo')
      return
    }
    if (!inField && editing.mode === 'none' && e.key === '/') {
      e.preventDefault()
      app.searchOpen = true
    }
  }

  // flash a block after opening it from search
  function flash() {
    const raw = sessionStorage.getItem('notatnik-flash')
    if (!raw) return
    sessionStorage.removeItem('notatnik-flash')
    const { page, path } = JSON.parse(raw) as { page: string; path: number[] | null }
    const doc = getDoc(page)
    const b = path && doc.view ? getBlock(doc.view.blocks, path) : undefined
    if (!b) return
    const el = document.querySelector(`[data-block-id="${b.id}"]`)
    el?.scrollIntoView({ block: 'center' })
    el?.classList.add('flash')
    setTimeout(() => el?.classList.remove('flash'), 1500)
  }
  onMount(() => {
    window.addEventListener('notatnik-flash', flash)
    return () => window.removeEventListener('notatnik-flash', flash)
  })
</script>

{#if app.needsGraph}
  <Welcome />
{:else}
  <div class="app" class:left-closed={!app.leftOpen}>
    <aside class="left" aria-label="Sidebar">
      <nav>
        <a href="#/journals" class:active={route.kind === 'journals'}>Journals</a>
        <a href="#/agenda" class:active={route.kind === 'agenda'}>Agenda</a>
        <button class="link" onclick={() => (app.searchOpen = true)}>Search</button>
        <a href="#/settings">Settings</a>
      </nav>
      <div class="status" role="status" data-testid="save-status" data-status={overallStatus()}>
        {overallStatus() === 'saved' ? '✓ saved' : overallStatus() === 'saving' ? '… saving' : '⚠ not saved'}
      </div>
      {#if saved.length}
        <h3>Saved queries</h3>
        <ul class="saved">
          {#each saved as s (s.name)}<li>
              <a href={`#/saved/${encodeURIComponent(s.name)}`}>{s.name}</a>
            </li>{/each}
        </ul>
      {/if}
      <h3>Pages</h3>
      <NamespaceTree />
    </aside>
    <main>
      <ConflictBanner />
      {#if route.kind === 'journals'}
        <JournalsView />
      {:else if route.kind === 'settings'}
        <Settings />
      {:else if route.kind === 'agenda'}
        <AgendaView />
      {:else if route.kind === 'saved'}
        {#key route.name}<SavedQueryPage name={route.name} />{/key}
      {:else}
        {#key route.name}
          <PageContent name={route.name} viewKey="main" zoom={route.zoom} />
        {/key}
      {/if}
    </main>
    {#if app.rightPanels.length}
      <aside class="right" aria-label="Side panels">
        {#each app.rightPanels as p (p)}
          <section class="panel">
            <header>
              <a href={pageHash(p)}>{p}</a><button
                aria-label="Close panel"
                onclick={() => (app.rightPanels = app.rightPanels.filter((x) => x !== p))}>×</button
              >
            </header>
            <PageContent name={p} viewKey={`side-${p}`} showBacklinks={false} compact />
          </section>
        {/each}
      </aside>
    {/if}
  </div>
{/if}
{#if app.searchOpen}<SearchDialog />{/if}
{#if app.paletteOpen}<Palette />{/if}
<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}<div class="toast {t.kind}" role={t.kind === 'error' ? 'alert' : 'status'}>
      {t.message}
    </div>{/each}
</div>
