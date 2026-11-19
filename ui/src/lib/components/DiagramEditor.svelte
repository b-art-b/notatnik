<script lang="ts">
  // The diagram editor (DG-1 to DG-4, DG-8, DG-10): Excalidraw in a full-screen dialog, the sibling of `DrawingEditor`
  // (handwriting). Excalidraw is a React component, so it is not rendered by Svelte: `mountDiagram` (diagram/excalidraw.ts)
  // loads it when this dialog opens, mounts it into the `<div>` that `bind:this` gives us, and is taken apart again
  // when the component goes away. What it knows is read when a button is pressed, not tracked as state.
  //
  // The header (Name, Picture / Link, Delete…, Cancel, Done), the draft and the way a new or an existing file is
  // saved are the drawing editor's, kept in step with it on purpose (same `save_drawing`, same `DrawingHost`).
  import { onMount } from 'svelte'
  import { backend } from '../api'
  import { app, toast, errorMessage } from '../stores/app.svelte'
  import {
    drawingMarkdown,
    finishDrawing,
    type DrawingMode,
    type DrawingOpen,
    type DrawingResult,
  } from '../commands/drawing'
  import { DraftSaver, draftKey, idbStore, type Draft, type DraftStore } from '../drawing/draft'
  import { mountDiagram, type DiagramHandle } from '../diagram/excalidraw'

  // `app.drawing` is set when the dialog opens and not null while it is shown: a snapshot is enough
  const target: DrawingOpen = app.drawing ?? { page: null, path: null, name: '', mode: 'picture', host: null }
  const isNew = !target.path
  const key = draftKey(target, 'diagram')

  let dialog: HTMLDivElement | undefined = $state()
  let host: HTMLDivElement | undefined = $state()
  let handle: DiagramHandle | undefined
  let loading = $state(true)
  let saving = $state(false)
  // closing with changes asks first; deleting always asks
  let confirming = $state(false)
  let deleting = $state(false)
  // the label of the reference in the note, and picture or link; the block is only rewritten when Done is pressed
  let name = $state(target.name)
  let mode = $state<DrawingMode>(target.mode)
  // an existing diagram can only be renamed or switched where a block holds it (a table cell cannot rewrite one)
  const canDescribe = isNew || !!target.host
  // a draft of this diagram that was kept (DR-13, DG-8)
  let draft = $state<Draft | null>(null)
  // a draft that was restored is a change that is not in the editor's own history
  let forceDirty = false
  let drafts: DraftStore | undefined
  let saver: DraftSaver | undefined
  let loadError = $state('')
  // the file has no Excalidraw scene: Done stays off until the person chooses to replace it with a new diagram
  let noScene = $state(false)
  // the copies a sync left of this diagram when it was changed on two devices (DR-23, DG-9), and the ones whose
  // content is in the editor now: they go to the trash when the diagram is saved
  let copies = $state<string[]>([])
  let consumed: string[] = []

  const theme = (): 'light' | 'dark' => (document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light')
  const metaChanged = () => name.trim() !== target.name.trim() || mode !== target.mode
  const strokesChanged = () => forceDirty || (!!handle && handle.changed())
  /** Something would be lost by closing: shapes, or for an existing diagram a new name or form. */
  const dirty = () => (isNew ? !!handle && !handle.isEmpty() : strokesChanged() || metaChanged())

  async function mountWith(svg: string | undefined) {
    if (!host) return
    handle?.unmount()
    handle = await mountDiagram(host, { svg, theme: theme(), onChange: () => saver?.touch() })
  }

  onMount(() => {
    const hide = () => {
      if (document.visibilityState === 'hidden') void saver?.flush()
    }
    document.addEventListener('visibilitychange', hide)
    window.addEventListener('pagehide', hide)
    void (async () => {
      try {
        let svg: string | undefined
        if (target.path) svg = (await backend.call('read_drawing', { path: target.path })).value
        await mountWith(svg)
        noScene = !!handle?.noScene
      } catch (e) {
        loadError = errorMessage(e)
        loading = false
        return
      }
      loading = false
      dialog?.focus()
      drafts = idbStore()
      saver = new DraftSaver(drafts, key, () => (dirty() && handle ? handle.getSvg() : null))
      try {
        const found = await drafts.get(key)
        if (found?.svg) draft = found
      } catch {
        /* no storage: nothing was kept */
      }
      if (target.path) {
        try {
          copies = (await backend.call('drawing_copies', { path: target.path })).items
        } catch {
          /* an older backend, or none: no copies are offered */
        }
      }
    })()
    return () => {
      document.removeEventListener('visibilitychange', hide)
      window.removeEventListener('pagehide', hide)
      handle?.unmount()
      handle = undefined
    }
  })

  /** Closes the editor: the draft goes (what it kept is saved or thrown away now). */
  async function leave(result: DrawingResult | null) {
    await saver?.clear()
    finishDrawing(result)
  }

  async function restoreDraft() {
    const d = draft
    draft = null
    if (!d) return
    loading = true
    try {
      await mountWith(d.svg)
      forceDirty = true
      saver?.touch()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
    loading = false
    dialog?.focus()
  }
  async function dropDraft() {
    draft = null
    try {
      await drafts?.delete(key)
    } catch {
      /* it was not there */
    }
    dialog?.focus()
  }

  /** "Use that version": the editor shows the other device's diagram; this device's file is overwritten on Done and the
   * copy goes to the trash then. There is no merge inside a diagram. */
  async function useCopy(copy: string) {
    loading = true
    try {
      await mountWith((await backend.call('read_drawing', { path: copy })).value)
      forceDirty = true
      consumed.push(copy)
      copies = copies.filter((c) => c !== copy)
      saver?.touch()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
    loading = false
    dialog?.focus()
  }
  /** "Keep this one": the other version is not wanted; its copy goes to the trash at once. */
  async function keepMine(copy: string) {
    try {
      await backend.call('discard_drawing_copy', { path: copy })
      copies = copies.filter((c) => c !== copy)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
    dialog?.focus()
  }
  /** "Keep both": the copy stays beside the diagram as a file; this window stops asking about it. */
  function keepBoth(copy: string) {
    copies = copies.filter((c) => c !== copy)
    dialog?.focus()
  }
  /** The date and time of a copy, from its name (`… (conflicted copy 2026-10-03 120000).svg`). */
  const copyWhen = (p: string) => /\(conflicted copy ([^)]*)\)/.exec(p)?.[1] ?? ''

  /** Done: a new diagram that is empty, or an old one with nothing changed, stores nothing. The file is written when
   * the shapes changed; a new name or form only rewrites the reference in the block. */
  async function done() {
    const h = handle
    if (!h || saving || loading) return
    if (isNew ? h.isEmpty() : !strokesChanged() && !metaChanged()) {
      await leave(null)
      return
    }
    saving = true
    try {
      let path = target.path
      let markdown = ''
      if (isNew || strokesChanged()) {
        const svg = await h.getSvg()
        const saved = await backend.call('save_drawing', {
          page: target.page,
          path: target.path,
          svg,
          kind: 'diagram',
        })
        path = saved.path
        h.markSaved()
        forceDirty = false
        // `![](…)` as the server wrote it: the path as this page links it, to write again with a name
        if (isNew) {
          const href = /^!\[\]\((.*)\)$/.exec(saved.markdown)?.[1] ?? saved.path
          markdown = drawingMarkdown(href, name, mode)
        }
      }
      // an existing diagram: the block writes the new name and form around the target it already has
      if (!isNew && target.host && path && metaChanged())
        await target.host.replace(path, (href) => drawingMarkdown(href, name, mode))
      // the copies whose content is in the diagram now are not needed any more
      for (const c of consumed) await backend.call('discard_drawing_copy', { path: c }).catch(() => undefined)
      await leave({ path: path ?? '', markdown })
    } catch (e) {
      toast(errorMessage(e), 'error')
      saving = false
    }
  }

  /** Delete (after asking): the reference goes out of the block, then the file to the trash if no other page
   * links it. */
  async function remove() {
    const path = target.path
    if (!path || !target.host || saving) return
    saving = true
    try {
      await target.host.replace(path, null)
      const unused = (await backend.call('list_orphan_assets', {})).items.some((i) => i.path === path)
      if (unused) await backend.call('delete_orphan_assets', { paths: [path] })
      else toast('Another page links this diagram too, so its file stays.')
      await leave({ path, markdown: '', deleted: true })
    } catch (e) {
      toast(errorMessage(e), 'error')
      saving = false
    }
  }

  /** Cancel, and Escape: closes at once when nothing changed, else asks. */
  function cancel() {
    if (saving) return
    if (dirty()) {
      confirming = true
      return
    }
    void leave(null)
  }

  function onkey(e: KeyboardEvent) {
    // Excalidraw's keys (undo, delete, tool letters ...) are its own; the page behind must not see any of them
    e.stopPropagation()
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault()
      if (confirming || deleting) {
        confirming = false
        deleting = false
        dialog?.focus()
      } else cancel()
    }
  }
</script>

<div class="overlay" role="presentation">
  <div
    class="dialog"
    role="dialog"
    aria-label="Diagram"
    aria-modal="true"
    tabindex="-1"
    data-testid="diagram-editor"
    bind:this={dialog}
    onkeydown={onkey}
  >
    <header>
      <h2>Diagram</h2>
      <label class="name">
        <span class="sr">Name</span>
        <input
          bind:value={name}
          placeholder="Name (optional)"
          maxlength="120"
          disabled={!canDescribe}
          title={canDescribe ? '' : 'The name is changed where the diagram is written in a block'}
          data-testid="diagram-name"
          aria-label="Name"
        />
      </label>
      <fieldset class="mode" disabled={!canDescribe}>
        <legend class="sr">Show in the note as</legend>
        <label title="The diagram is shown in the note" class:on={mode === 'picture'}
          ><input type="radio" name="diagram-mode" value="picture" bind:group={mode} /> Picture</label
        >
        <label title="Only a link is written; a click opens the diagram" class:on={mode === 'link'}
          ><input type="radio" name="diagram-mode" value="link" bind:group={mode} /> Link</label
        >
      </fieldset>
      <span class="spacer"></span>
      <div class="actions">
        {#if !isNew && target.host}
          <button
            type="button"
            class="plain danger"
            onclick={() => (deleting = true)}
            disabled={saving}
            data-testid="diagram-delete">Delete…</button
          >
        {/if}
        <button type="button" class="plain" onclick={cancel} data-testid="diagram-cancel">Cancel</button>
        <button
          type="button"
          onclick={done}
          disabled={loading || saving || !!loadError || noScene}
          data-testid="diagram-done">{saving ? 'Saving…' : 'Done'}</button
        >
      </div>
    </header>
    {#if deleting}
      <div
        class="confirm"
        role="alertdialog"
        aria-label="Delete the diagram?"
        data-testid="diagram-delete-confirm"
      >
        <span>Delete this diagram? It is removed from the note and its file goes to the trash.</span>
        <button
          type="button"
          data-testid="diagram-delete-keep"
          onclick={() => {
            deleting = false
            dialog?.focus()
          }}>Keep it</button
        >
        <button type="button" class="danger" data-testid="diagram-delete-yes" onclick={remove}>Delete</button>
      </div>
    {/if}
    {#if confirming}
      <div class="confirm" role="alertdialog" aria-label="Discard the diagram?" data-testid="diagram-confirm">
        <span>Discard what you drew?</span>
        <button
          type="button"
          data-testid="diagram-keep"
          onclick={() => {
            confirming = false
            // the button that had the focus is gone: give it back to the dialog, or Escape would not reach us
            dialog?.focus()
          }}>Keep drawing</button
        >
        <button type="button" class="danger" data-testid="diagram-discard" onclick={() => void leave(null)}
          >Discard</button
        >
      </div>
    {/if}
    {#if draft}
      <!-- DG-8: what was being drawn when the app stopped -->
      <div class="confirm banner" role="status" data-testid="diagram-draft-banner">
        <span
          >A diagram that was not saved was kept ({new Date(draft.savedAt).toLocaleString()}). Restore it?</span
        >
        <button type="button" data-testid="diagram-draft-restore" onclick={restoreDraft}>Restore</button>
        <button type="button" data-testid="diagram-draft-discard" onclick={dropDraft}>Discard it</button>
      </div>
    {/if}
    {#if copies.length}
      <!-- DG-9: this diagram was changed on two devices; the sync kept the other version beside it -->
      <div class="confirm banner" role="status" data-testid="diagram-copy-banner">
        <span
          >This diagram was changed on two devices. The other version ({copyWhen(copies[0])}) was saved as a
          copy{copies.length > 1 ? `, and ${copies.length - 1} more` : ''}.</span
        >
        <button type="button" data-testid="diagram-copy-keep" onclick={() => void keepMine(copies[0])}
          >Keep this one</button
        >
        <button type="button" data-testid="diagram-copy-use" onclick={() => void useCopy(copies[0])}
          >Use that version</button
        >
        <button type="button" data-testid="diagram-copy-both" onclick={() => keepBoth(copies[0])}
          >Keep both</button
        >
      </div>
    {/if}
    {#if noScene}
      <!-- the picture is fine, but it has no shapes to edit: say so instead of showing a blank page that would replace it -->
      <div class="confirm banner" role="alert" data-testid="diagram-noscene">
        <span
          >This file has no editable diagram in it (another program may have saved it again). A new diagram
          here would replace the picture.</span
        >
        <button
          type="button"
          class="danger"
          data-testid="diagram-noscene-replace"
          onclick={() => {
            noScene = false
            dialog?.focus()
          }}>Start a new diagram</button
        >
      </div>
    {/if}
    {#if loadError}
      <div class="confirm" role="alert" data-testid="diagram-error">
        <span>This diagram could not be opened: {loadError}</span>
      </div>
    {/if}
    <!-- React renders Excalidraw into this div; Svelte leaves what is inside alone -->
    <div class="host" bind:this={host} class:loading data-testid="diagram-host"></div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    z-index: 60;
    display: flex;
  }
  .dialog:focus {
    outline: none;
  }
  .dialog {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    margin: 2vh 2vw;
    min-height: 0;
    background: var(--panel);
    border-radius: 10px;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.8em;
    padding: 0.5em 0.9em;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 1.05em;
  }
  .spacer {
    flex: 1;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .name input {
    font: inherit;
    padding: 0.3em 0.6em;
    width: 15em;
    max-width: 40vw;
    color: inherit;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  /* Picture | Link as one segmented control; the radio itself is hidden, the label is the target */
  .mode {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    margin: 0;
    padding: 2px;
    gap: 2px;
  }
  .mode label {
    position: relative;
    padding: 0.25em 0.9em;
    border-radius: 6px;
    cursor: pointer;
  }
  .mode label.on {
    background: var(--chip);
    font-weight: 600;
  }
  .mode label:has(input:focus-visible) {
    outline: 2px solid var(--accent);
  }
  .mode input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .mode:disabled {
    opacity: 0.6;
  }
  .actions {
    display: flex;
    gap: 0.5em;
  }
  .actions .danger {
    color: var(--danger);
  }
  .actions button,
  .confirm button {
    font: inherit;
    padding: 0.35em 1.1em;
    border-radius: 8px;
    cursor: pointer;
    border: 1px solid var(--border);
    background: transparent;
    color: inherit;
  }
  .actions button:not(.plain) {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .actions button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .confirm {
    display: flex;
    align-items: center;
    gap: 0.6em;
    padding: 0.5em 0.9em;
    background: var(--chip);
    border-bottom: 1px solid var(--border);
  }
  .confirm span {
    flex: 1;
  }
  .confirm .danger {
    color: var(--danger);
    border-color: var(--danger);
  }
  /* Excalidraw fills this box (its container is 100 % high) */
  .host {
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .host.loading {
    opacity: 0.5;
  }
  @media (pointer: coarse) {
    .actions button {
      min-height: 44px;
    }
  }
</style>
