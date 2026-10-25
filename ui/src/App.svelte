<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import './app.css'
  import {
    app,
    bootstrap,
    dismissToast,
    LEFT_DEFAULT,
    onSide,
    RIGHT_DEFAULT,
    setLeftWidth,
    setRightWidth,
    toggleLeft,
    toggleRight,
    toggleRightFocus,
    PAGE_MIN,
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
  import GraphSwitcher from './lib/components/GraphSwitcher.svelte'
  import SidePanel from './lib/components/SidePanel.svelte'
  import SyncIndicator from './lib/components/SyncIndicator.svelte'
  import CalendarAuto from './lib/components/CalendarAuto.svelte'
  import FavoritePages from './lib/components/FavoritePages.svelte'
  import RecentPages from './lib/components/RecentPages.svelte'
  import { installDragSelect } from './lib/dragSelect'
  import { touchRecent } from './lib/stores/recent.svelte'
  import { loadIcons } from './lib/stores/icons.svelte'
  import { refreshWorkspace } from './lib/stores/workspace.svelte'
  import SearchDialog from './lib/components/SearchDialog.svelte'
  import TemplateAskDialog from './lib/components/TemplateAskDialog.svelte'
  import SnippetDialog from './lib/components/SnippetDialog.svelte'
  import TransferDialog from './lib/components/TransferDialog.svelte'
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
  import { initZoom, zoomKey, zoomWheel } from './lib/zoom'
  import { installNavigation } from './lib/mouse'

  const route = $derived(app.route)
  // The right panel: this page's references and tasks (when Settings put them there), and the pages opened beside it
  const hasContext = $derived(route.kind === 'page' && (onSide('refs') || onSide('tasks')))
  const rightExists = $derived(hasContext || app.rightPanels.length > 0)
  const rightShown = $derived(rightExists && app.rightOpen)
  // the page is folded away: the panel has the window
  const focus = $derived(rightShown && app.rightFocus)
  let saved = $state<SavedQueryView[]>([])
  $effect(() => {
    void app.savedVersion
    void app.graph
    if (app.graph) backend.call('list_saved_queries', {}).then((l) => (saved = l))
  })

  // the graphs of this device, for the "To another graph" dialog and its button (read again for each graph that opens)
  $effect(() => {
    void app.graph?.id
    void refreshWorkspace()
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

  // Dragging the right panel's edge (the mirror of the sidebar's)
  let resizingRight = $state(false)
  function startResizeRight(e: PointerEvent) {
    e.preventDefault()
    const handle = e.currentTarget as HTMLElement
    handle.setPointerCapture(e.pointerId)
    resizingRight = true
    // no upper limit: dragged so far that the page would be narrower than PAGE_MIN, the page folds away; dragged
    // back, it returns
    const move = (m: PointerEvent) => {
      const want = window.innerWidth - m.clientX
      const room = window.innerWidth - (app.leftOpen ? app.leftWidth : 0) - PAGE_MIN
      app.rightFocus = want > room
      if (!app.rightFocus) setRightWidth(want, false)
    }
    const stop = () => {
      handle.removeEventListener('pointermove', move)
      handle.removeEventListener('pointerup', stop)
      handle.removeEventListener('pointercancel', stop)
      resizingRight = false
      setRightWidth(app.rightWidth) // remember where it ended
    }
    handle.addEventListener('pointermove', move)
    handle.addEventListener('pointerup', stop)
    handle.addEventListener('pointercancel', stop)
  }
  function resizeKeyRight(e: KeyboardEvent) {
    const step = e.shiftKey ? 40 : 10
    // the handle is on the panel's left edge: moving it left makes the panel wider
    if (e.key === 'ArrowLeft') {
      const room = window.innerWidth - (app.leftOpen ? app.leftWidth : 0) - PAGE_MIN
      if (app.rightWidth + step > room) app.rightFocus = true
      else setRightWidth(app.rightWidth + step)
    } else if (e.key === 'ArrowRight') {
      if (app.rightFocus) app.rightFocus = false
      else setRightWidth(app.rightWidth - step)
    } else if (e.key === 'Home') {
      app.rightFocus = false
      setRightWidth(RIGHT_DEFAULT)
    } else return
    e.preventDefault()
  }

  async function globalKey(e: KeyboardEvent) {
    if (zoomKey(e)) return
    const mod = e.ctrlKey || e.metaKey
    if (mod && e.shiftKey && e.code === 'Backslash') {
      e.preventDefault()
      toggleRight()
      return
    }
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
      // WebKitGTK names Shift+Tab `ISO_Left_Tab`; the physical key (`code`) is the same everywhere
      else if (e.code === 'Tab' || k === 'Tab' || k === 'ISO_Left_Tab')
        void (e.shiftKey || k === 'ISO_Left_Tab' ? selectionOutdent(ctx) : selectionIndent(ctx))
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
  <div
    class="app"
    class:left-closed={!app.leftOpen}
    class:resizing={resizing || resizingRight}
    class:right-focus={focus}
    style:--left-w="{app.leftWidth}px"
    style:--right-w="{app.rightWidth}px"
    style:--left-eff="{app.leftOpen ? app.leftWidth : 0}px"
  >
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
      <GraphSwitcher />
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
      <SyncIndicator />
      <CalendarAuto />
      <FavoritePages />
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
      {#if focus}
        <button
          class="show-page"
          onclick={toggleRightFocus}
          data-testid="show-page"
          title="Show the page again"><span aria-hidden="true">◂</span> Show page</button
        >
      {/if}
      <div class="column" hidden={focus}>
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
    {#if rightExists}
      <button
        class="right-toggle"
        class:open={rightShown}
        aria-label={rightShown ? 'Hide side panel' : 'Show side panel'}
        aria-expanded={rightShown}
        title={`${rightShown ? 'Hide' : 'Show'} the side panel (Ctrl+Shift+\\)`}
        onclick={toggleRight}
        data-testid="right-toggle"
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
          /><path d="M10 3v10" stroke="currentColor" stroke-width="1.5" /></svg
        ></button
      >
      {#if rightShown}
        <button
          class="right-toggle right-focus-btn"
          aria-label={focus ? 'Show the page' : 'Focus on the side panel'}
          aria-pressed={focus}
          title={focus ? 'Show the page again' : 'Fold the page away and give the panel the whole window'}
          onclick={toggleRightFocus}
          data-testid="right-focus"
          ><svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"
            ><path
              d={focus ? 'M2 6h4V2M14 6h-4V2M2 10h4v4M14 10h-4v4' : 'M6 2H2v4M10 2h4v4M6 14H2v-4M10 14h4v-4'}
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            /></svg
          ></button
        >
      {/if}
    {/if}
    {#if rightShown}
      <!-- a focusable separator on the panel's left edge, like the sidebar's -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="resizer resizer-right"
        class:active={resizingRight}
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize side panel"
        aria-valuemin={280}
        aria-valuenow={app.rightWidth}
        tabindex="0"
        data-testid="right-resizer"
        onpointerdown={startResizeRight}
        onkeydown={resizeKeyRight}
        ondblclick={toggleRightFocus}
      ></div>
      <aside class="right" aria-label="Side panels" data-testid="side-panel">
        <SidePanel page={route.kind === 'page' ? route.name : null} />
      </aside>
    {/if}
  </div>
{/if}
{#if app.searchOpen}<SearchDialog />{/if}
{#if app.paletteOpen}<Palette />{/if}
{#if app.snippetText !== null}<SnippetDialog />{/if}
{#if app.transfer}<TransferDialog />{/if}
{#if app.templateAsks}<TemplateAskDialog />{/if}
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
