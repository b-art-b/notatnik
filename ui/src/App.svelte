<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import './app.css'
  import {
    app,
    bootstrap,
    dismissToast,
    LEFT_DEFAULT,
    setLeftWidth,
    toggleLeft,
  } from './lib/stores/app.svelte'
  import { watchArchived } from './lib/stores/archived.svelte'
  import { editing, isEditingDoc, stopEditing } from './lib/stores/editing.svelte'
  import {
    flushAll,
    getDoc,
    handleExternalChange,
    overallStatus,
    reloadOpenDocs,
  } from './lib/stores/pages.svelte'
  import { bus } from './lib/stores/events.svelte'
  import {
    globalUndo,
    selectionDelete,
    selectionEdit,
    selectionIndent,
    selectionCopy,
    selectionCopyRefs,
    selectionCut,
    selectionCycleTask,
    selectionDuplicate,
    selectionPaste,
    selectAllBlocks,
    selectionMove,
    selectionOutdent,
    selectionStep,
  } from './lib/commands/exec'
  import { getBlock } from './lib/tree'
  import PageContent from './lib/components/PageContent.svelte'
  import JournalsView from './lib/components/JournalsView.svelte'
  import PagesPanel from './lib/components/PagesPanel.svelte'
  import RecentPages from './lib/components/RecentPages.svelte'
  import { installDragSelect } from './lib/dragSelect'
  import { touchRecent } from './lib/stores/recent.svelte'
  import { loadIcons } from './lib/stores/icons.svelte'
  import SearchDialog from './lib/components/SearchDialog.svelte'
  import Palette from './lib/components/Palette.svelte'
  import Settings from './lib/components/Settings.svelte'
  import Welcome from './lib/components/Welcome.svelte'
  import ConflictBanner from './lib/components/ConflictBanner.svelte'
  import AgendaView from './lib/components/AgendaView.svelte'
  import AllPagesView from './lib/components/AllPagesView.svelte'
  import PropertyPages from './lib/components/PropertyPages.svelte'
  import BlockProperties from './lib/components/BlockProperties.svelte'
  import SegmentView from './lib/components/SegmentView.svelte'
  import SavedQueryPage from './lib/components/SavedQueryPage.svelte'
  import { backend } from './lib/api'
  import type { SavedQueryView } from './lib/api'
  import { pageHash } from './lib/route'
  import { initZoom, zoomKey, zoomWheel } from './lib/zoom'
  import { installNavigation } from './lib/mouse'

  const route = $derived(app.route)
  let saved = $state<SavedQueryView[]>([])
  $effect(() => {
    void app.savedVersion
    void app.graph
    if (app.graph) backend.call('list_saved_queries', {}).then((l) => (saved = l))
  })

  // icons depend on page properties: refresh them (a little later, edits come in bursts) when pages change
  $effect(() => {
    void bus.version
    const t = setTimeout(() => void loadIcons(), 300)
    return () => clearTimeout(t)
  })

  // every page you open goes to the top of the "Recent" list
  $effect(() => {
    const r = route
    if (r.kind === 'page') untrack(() => touchRecent(r.name))
  })

  $effect(() => {
    void bus.version
    const e = bus.last
    // untracked: reloading documents reads their state, which must not re-trigger this effect
    untrack(() => {
      if (e && !e.own && e.pages.length) handleExternalChange(e.pages, isEditingDoc)
      // config.toml was edited outside the app: the labels, week start and icons may have changed
      if (e && !e.own && e.paths.includes('.notatnik/config.toml')) void configChanged()
    })
  })

  async function configChanged() {
    try {
      app.graph = await backend.call('graph_info', {})
      await loadIcons()
      await reloadOpenDocs()
    } catch {
      /* the next change tries again */
    }
  }

  onMount(() => {
    void bootstrap()
    watchArchived()
    initZoom()
    const removeNavigation = installNavigation()
    const onKey = (e: KeyboardEvent) => void globalKey(e)
    window.addEventListener('keydown', onKey, true)
    // not passive: zoomWheel calls preventDefault to stop the browser's own zoom
    window.addEventListener('wheel', zoomWheel, { passive: false })
    const beforeUnload = () => void flushAll()
    window.addEventListener('beforeunload', beforeUnload)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('wheel', zoomWheel)
      removeNavigation()
      window.removeEventListener('beforeunload', beforeUnload)
    }
  })

  // Dragging the sidebar's edge. Pointer capture keeps the events coming while the pointer is over other elements.
  let resizing = $state(false)
  function startResize(e: PointerEvent) {
    // no text selection and no native drag from under the pointer: either one cancels the pointer (`pointercancel`)
    // and the resize stops after its first move
    e.preventDefault()
    const handle = e.currentTarget as HTMLElement
    handle.setPointerCapture(e.pointerId)
    resizing = true
    const move = (m: PointerEvent) => setLeftWidth(m.clientX, false)
    const stop = () => {
      handle.removeEventListener('pointermove', move)
      handle.removeEventListener('pointerup', stop)
      handle.removeEventListener('pointercancel', stop)
      resizing = false
      setLeftWidth(app.leftWidth) // remember where it ended
    }
    handle.addEventListener('pointermove', move)
    handle.addEventListener('pointerup', stop)
    handle.addEventListener('pointercancel', stop)
  }
  function resizeKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 40 : 10
    if (e.key === 'ArrowLeft') setLeftWidth(app.leftWidth - step)
    else if (e.key === 'ArrowRight') setLeftWidth(app.leftWidth + step)
    else if (e.key === 'Home') setLeftWidth(LEFT_DEFAULT)
    else return
    e.preventDefault()
  }

  async function globalKey(e: KeyboardEvent) {
    if (zoomKey(e)) return
    const mod = e.ctrlKey || e.metaKey
    if (mod && e.key === '\\') {
      e.preventDefault()
      toggleLeft()
      return
    }
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
      else if (k === 'ArrowRight' && e.altKey) void selectionIndent(ctx)
      else if (k === 'ArrowLeft' && e.altKey) void selectionOutdent(ctx)
      else if (k === 'Delete' || k === 'Backspace') void selectionDelete(ctx)
      else if (k === 'Enter' && !mod) selectionEdit(ctx)
      else if (k === 'Escape') stopEditing()
      else if (mod && e.shiftKey && k.toLowerCase() === 'c') void selectionCopyRefs(ctx)
      else if (mod && k.toLowerCase() === 'c') void selectionCopy(ctx)
      else if (mod && k.toLowerCase() === 'x') void selectionCut(ctx)
      else if (mod && k.toLowerCase() === 'd') void selectionDuplicate(ctx)
      else if (mod && k.toLowerCase() === 'a') selectAllBlocks(ctx)
      else if (mod && k === 'Enter') void selectionCycleTask(ctx)
      else handled = false
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
    window.dispatchEvent(new CustomEvent('notatnik-render-all'))
    const { page, path } = JSON.parse(raw) as { page: string; path: number[] | null }
    const doc = getDoc(page)
    const b = path && doc.view ? getBlock(doc.view.blocks, path) : undefined
    if (!b) return
    const el = document.querySelector(`[data-block-id="${b.id}"]`)
    el?.scrollIntoView({ block: 'center' })
    el?.classList.add('flash')
    setTimeout(() => el?.classList.remove('flash'), 1500)
  }
  // Ctrl+V with blocks selected (not while typing in a field): the copied blocks go below the selection
  function pasteOnSelection(e: ClipboardEvent) {
    const t = e.target
    if (
      editing.mode !== 'select' ||
      (t instanceof Element && t.closest('input, textarea, select, .cm-editor'))
    )
      return
    const text = e.clipboardData?.getData('text/plain') ?? ''
    if (!text.trim()) return
    e.preventDefault()
    void selectionPaste({ doc: getDoc(editing.docName), viewKey: editing.viewKey }, text)
  }
  onMount(() => {
    window.addEventListener('notatnik-flash', flash)
    window.addEventListener('paste', pasteOnSelection)
    const removeDrag = installDragSelect()
    return () => {
      window.removeEventListener('notatnik-flash', flash)
      window.removeEventListener('paste', pasteOnSelection)
      removeDrag()
    }
  })
</script>

{#if app.needsGraph}
  <Welcome />
{:else}
  <div class="app" class:left-closed={!app.leftOpen} class:resizing style:--left-w="{app.leftWidth}px">
    <!-- one button for both states: it sits on the sidebar's edge while open and in the corner when closed -->
    <button
      class="left-toggle"
      aria-label={app.leftOpen ? 'Hide sidebar' : 'Show sidebar'}
      aria-expanded={app.leftOpen}
      title={`${app.leftOpen ? 'Hide' : 'Show'} the sidebar (Ctrl+\\)`}
      onclick={toggleLeft}
      data-testid="left-toggle"
      ><svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"
        ><rect
          x="1.75"
          y="2.75"
          width="12.5"
          height="10.5"
          rx="2"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
        /><path d="M6 3v10" stroke="currentColor" stroke-width="1.5" /></svg
      ></button
    >
    <aside class="left" aria-label="Sidebar">
      <nav>
        <a href="#/journals" class:active={route.kind === 'journals'}
          ><span class="ico ico-journals" aria-hidden="true"></span>Journals</a
        >
        <a href="#/agenda" class:active={route.kind === 'agenda'}
          ><span class="ico ico-agenda" aria-hidden="true"></span>Agenda</a
        >
        <a href="#/pages" class:active={route.kind === 'pages'}
          ><span class="ico ico-pages" aria-hidden="true"></span>All pages</a
        >
        <button class="link" onclick={() => (app.searchOpen = true)}
          ><span class="ico ico-search" aria-hidden="true"></span>Search</button
        >
        <a href="#/settings" class:active={route.kind === 'settings'}
          ><span class="ico ico-settings" aria-hidden="true"></span>Settings</a
        >
      </nav>
      <div class="status" role="status" data-testid="save-status" data-status={overallStatus()}>
        {overallStatus() === 'saved' ? '✓ saved' : overallStatus() === 'saving' ? '… saving' : '⚠ not saved'}
      </div>
      <RecentPages />
      {#if saved.length}
        <h3>Saved queries</h3>
        <ul class="saved">
          {#each saved as s (s.name)}<li>
              <a href={`#/saved/${encodeURIComponent(s.name)}`}>{s.name}</a>
            </li>{/each}
        </ul>
      {/if}
      <PagesPanel />
    </aside>
    <!-- a focusable separator: drag it, or use the arrow keys (Home resets) -->
    {#if app.leftOpen}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="resizer"
        class:active={resizing}
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize sidebar"
        aria-valuemin={180}
        aria-valuemax={560}
        aria-valuenow={app.leftWidth}
        tabindex="0"
        data-testid="left-resizer"
        onpointerdown={startResize}
        onkeydown={resizeKey}
        ondblclick={() => setLeftWidth(LEFT_DEFAULT)}
      ></div>
    {/if}
    <main>
      <div class="column">
        <ConflictBanner />
        {#if route.kind === 'journals'}
          <JournalsView />
        {:else if route.kind === 'settings'}
          <Settings />
        {:else if route.kind === 'agenda'}
          <AgendaView />
        {:else if route.kind === 'pages'}
          <AllPagesView />
        {:else if route.kind === 'segment'}
          {#key route.name}<SegmentView segment={route.name} />{/key}
        {:else if route.kind === 'property'}
          {#key `${route.key}\u0000${route.value}`}<PropertyPages key={route.key} value={route.value} />{/key}
        {:else if route.kind === 'blockprop'}
          {#key `${route.key}\u0000${route.value}`}<BlockProperties
              key={route.key}
              value={route.value}
            />{/key}
        {:else if route.kind === 'saved'}
          {#key route.name}<SavedQueryPage name={route.name} />{/key}
        {:else}
          {#key route.name}
            <PageContent name={route.name} viewKey="main" zoom={route.zoom} />
          {/key}
        {/if}
      </div>
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
      {#if t.action}
        <button
          class="toast-action"
          onclick={() => {
            const run = t.action!.run
            dismissToast(t.id)
            run()
          }}>{t.action.label}</button
        >
      {/if}
    </div>{/each}
</div>
