<script lang="ts">
  // The right panel: one tab each for the page's references and tasks (when Settings put them here) and for the
  // pages opened beside it with Shift+click. All tabs stay mounted so that their badges are live; only the
  // chosen one is shown. With a single tab there is no tab bar.
  import { app, onSide, setRightTab } from '../stores/app.svelte'
  import { dockedNow } from '../stores/dock.svelte'
  import { finishPdf } from '../commands/pdf'
  import { finishBook } from '../commands/epub'
  import { pageHash } from '../route'
  import Backlinks from './Backlinks.svelte'
  import PageTasks from './PageTasks.svelte'
  import PageContent from './PageContent.svelte'

  let { page }: { page: string | null } = $props()

  type Tab = 'refs' | 'tasks' | 'pages' | 'document'
  let refCount = $state(0)
  let taskCount = $state(0)

  const tabs = $derived.by(() => {
    const t: { id: Tab; label: string; n: number | null }[] = []
    if (page !== null && onSide('refs')) t.push({ id: 'refs', label: 'References', n: refCount })
    if (page !== null && onSide('tasks')) t.push({ id: 'tasks', label: 'Tasks', n: taskCount })
    if (app.rightPanels.length) t.push({ id: 'pages', label: 'Pages', n: app.rightPanels.length })
    // a document docked beside the notes (DD-1, DD-4): one tab, whose body is where the viewer is drawn
    if (dockedNow()) t.push({ id: 'document', label: app.docTitle || 'Document', n: null })
    return t
  })
  // the remembered tab may not exist on this page (no side tasks, nothing opened): the first one is shown then
  const current = $derived<Tab>(
    tabs.some((t) => t.id === app.rightTab) ? app.rightTab : (tabs[0]?.id ?? 'refs'),
  )

  // The viewer is not drawn here: it stays where it is mounted (so docking reloads nothing) and is placed over this slot
  // by its rectangle, which is kept up to date every frame while the tab is shown (a resize of the panel, the sidebar
  // opening or a scroll of the window moves it without an event of its own).
  let slot = $state<HTMLDivElement | null>(null)
  $effect(() => {
    const el = slot
    if (!el || current !== 'document') {
      app.dockRect = null
      return
    }
    let raf = 0
    let last = ''
    const follow = () => {
      const b = el.getBoundingClientRect()
      const r = {
        left: Math.round(b.left),
        top: Math.round(b.top),
        width: Math.round(b.width),
        height: Math.round(b.height),
      }
      const key = `${r.left},${r.top},${r.width},${r.height}`
      if (key !== last) {
        last = key
        app.dockRect = r
      }
      raf = requestAnimationFrame(follow)
    }
    follow()
    return () => {
      cancelAnimationFrame(raf)
      app.dockRect = null
    }
  })

  /** Arrow keys move between tabs (the tab list is one stop in the tab order). */
  function onkeydown(e: KeyboardEvent) {
    const i = tabs.findIndex((t) => t.id === current)
    const to =
      e.key === 'ArrowRight'
        ? i + 1
        : e.key === 'ArrowLeft'
          ? i - 1
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? tabs.length - 1
              : -1
    if (to < 0) return
    e.preventDefault()
    const next = tabs[(to + tabs.length) % tabs.length]
    setRightTab(next.id)
    document.getElementById(`side-tab-${next.id}`)?.focus()
  }
</script>

{#if tabs.length > 1}
  <!-- svelte-ignore a11y_interactive_supports_focus (the tab buttons inside are the focus stops) -->
  <div class="tabs" role="tablist" aria-label="Side panel" {onkeydown}>
    {#each tabs as t (t.id)}
      <button
        id={`side-tab-${t.id}`}
        role="tab"
        aria-selected={current === t.id}
        aria-controls={`side-pane-${t.id}`}
        tabindex={current === t.id ? 0 : -1}
        class:on={current === t.id}
        onclick={() => setRightTab(t.id)}
        >{#if t.id === 'document'}<span class="doc-name">{t.label}</span
          >{:else}{t.label}{/if}{#if t.n !== null}<span class="badge">{t.n}</span>{/if}</button
      >{#if t.id === 'document'}<button
          type="button"
          class="close-tab"
          aria-label="Close the document"
          title="Close the document"
          data-testid="doc-tab-close"
          onclick={() => (app.pdf ? finishPdf() : finishBook())}>×</button
        >{/if}
    {/each}
  </div>
{/if}

{#if page !== null}
  {#key page}
    {#if onSide('tasks')}
      <div id="side-pane-tasks" role={tabs.length > 1 ? 'tabpanel' : undefined} hidden={current !== 'tasks'}>
        <PageTasks {page} bind:count={taskCount} />
      </div>
    {/if}
    {#if onSide('refs')}
      <div id="side-pane-refs" role={tabs.length > 1 ? 'tabpanel' : undefined} hidden={current !== 'refs'}>
        <Backlinks {page} bind:count={refCount} />
      </div>
    {/if}
  {/key}
{/if}
{#if dockedNow()}
  <div
    id="side-pane-document"
    class="doc-slot"
    role={tabs.length > 1 ? 'tabpanel' : undefined}
    hidden={current !== 'document'}
    bind:this={slot}
    data-testid="doc-slot"
  ></div>
{/if}
{#if app.rightPanels.length}
  <div id="side-pane-pages" role={tabs.length > 1 ? 'tabpanel' : undefined} hidden={current !== 'pages'}>
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
  </div>
{/if}

<style>
  .doc-slot {
    flex: 1;
    min-height: 0;
  }
  .doc-name {
    max-width: 12em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tabs .close-tab {
    padding: 0.3em 0.4em;
  }
  .tabs {
    display: flex;
    gap: 0.2em;
    margin-bottom: 0.8em;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: 0.4em;
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    padding: 0.3em 0.6em;
    margin-bottom: -1px;
    font: inherit;
    cursor: pointer;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.on {
    color: var(--text);
    font-weight: 600;
    border-bottom-color: var(--accent);
  }
  .badge {
    background: var(--chip);
    color: var(--muted);
    border-radius: 999px;
    padding: 0 0.5em;
    font-size: 0.75em;
    line-height: 1.7;
    font-weight: 400;
  }
</style>
