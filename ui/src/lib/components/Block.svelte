<script lang="ts">
  import { timeLabel } from '../clock'
  import TaskChain from './TaskChain.svelte'
  import BlockMenu from './BlockMenu.svelte'
  import type { BlockView } from '../api'
  import { editing, editingPath, startEditing, stopEditing } from '../stores/editing.svelte'
  import { getDoc } from '../stores/pages.svelte'
  import { dnd, endDrag } from '../stores/dnd.svelte'
  import { dropMove, samePath, toMarkdown, type DropZone } from '../tree'
  import { navigate, toast } from '../stores/app.svelte'
  import {
    commitText,
    editorAction,
    globalUndo,
    keepMine,
    pasteBlocks,
    setProperty,
    setTaskState,
    takeTheirs,
    type Ctx,
    copyText,
    ensureBlockUid,
    selectRangeTo,
    toggleInSelection,
    selectionCopy,
    selectionCut,
    selectionDelete,
    selectionDuplicate,
    selectionCopyRefs,
  } from '../commands/exec'
  import { app } from '../stores/app.svelte'
  import { dueIndicator, isOverdue, nextTaskState, progress } from '../tasks'
  import { shownDate } from '../stores/format.svelte'
  import Nodes from './Nodes.svelte'
  import Editor from './Editor.svelte'
  import DatePicker from './DatePicker.svelte'
  import PropertyCard from './PropertyCard.svelte'
  import { shownProps } from '../props'
  import Self from './Block.svelte'
  import { isLogOpen, logKey, toggleLog } from '../stores/logbook.svelte'

  interface Props {
    block: BlockView
    docName: string
    viewKey: string
    readonly?: boolean
    chain?: string[]
    /** Pages and journals pass `true`: with the "compact" task-details setting the schedule, repeat and logbook
     * become small indicators. Agenda, query results and linked references leave it off and show everything. */
    compact?: boolean
  }
  let { block, docName, viewKey, readonly = false, chain = [], compact = false }: Props = $props()

  // two independent device settings: task details (dates, repeat, logbook) and block properties (the card)
  const taskCompact = $derived(
    compact && app.taskDetails === 'compact' && !!(block.task || block.logbook.length),
  )
  const propCount = $derived(shownProps(block).length)
  const propsCompact = $derived(compact && app.blockProps === 'compact' && propCount > 0)
  const isCompact = $derived(taskCompact || propsCompact)
  // the details popover of a compact row: opened by hover or focus, kept open by a click (`pinned`)
  let detailsOpen = $state(false)
  let pinned = $state(false)
  const due = $derived(block.task ? dueIndicator(block.task, app.today) : null)
  function closeDetails() {
    if (!pinned && !chipEdit) detailsOpen = false
  }

  const doc = $derived(readonly ? null : getDoc(docName))
  const ctx = $derived<Ctx | null>(doc ? { doc, viewKey } : null)
  const isEditing = $derived(
    !readonly &&
      editing.mode === 'edit' &&
      editing.viewKey === viewKey &&
      editing.docName === docName &&
      editing.blockId === block.id,
  )
  const isSelected = $derived(
    !readonly &&
      editing.mode === 'select' &&
      editing.viewKey === viewKey &&
      editing.docName === docName &&
      editing.selection.includes(block.id),
  )
  // On the path to the block being edited (ancestors and the block itself): drawn as an elbow connector
  // from the parent's guide line, like Logseq's. Plain `$derived`: it only reads state.
  const onPath = $derived.by(() => {
    if (readonly || editing.mode !== 'edit' || editing.viewKey !== viewKey || editing.docName !== docName)
      return false
    const target = editingPath()
    const mine = block.path
    return mine.length > 0 && mine.length <= target.length && mine.every((x, i) => x === target[i])
  })
  // --- drag and drop: the bullet is the handle; the row under the pointer shows where the block would land ---
  const dragging = $derived(!!dnd.from && dnd.from.docName === docName && samePath(dnd.from.path, block.path))
  const dropZone = $derived(
    dnd.over && dnd.over.viewKey === viewKey && samePath(dnd.over.path, block.path) ? dnd.over.zone : null,
  )

  function startDrag(e: DragEvent) {
    if (readonly || !e.dataTransfer) return
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('application/x-notatnik-block', JSON.stringify({ docName, path: block.path }))
    const row = (e.currentTarget as HTMLElement).closest('.row')
    if (row) e.dataTransfer.setDragImage(row, 12, 12)
    dnd.from = { docName, path: [...block.path] }
  }

  /** Upper half: before. Lower half: first child if this block shows children, else after it, or a child when the
   * pointer is clearly to the right of the bullet (like Logseq). */
  function zoneAt(e: DragEvent): DropZone {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    if ((e.clientY - r.top) / r.height < 0.5) return 'before'
    if (hasChildren && !block.collapsed) return 'child'
    return e.clientX - r.left > 60 ? 'child' : 'after'
  }

  function dragOver(e: DragEvent) {
    const from = dnd.from
    if (readonly || !from || from.docName !== docName) return
    const zone = zoneAt(e)
    if (!dropMove(from.path, block.path, zone)) {
      dnd.over = null // nothing would change here: no drop line, no drop
      return
    }
    e.preventDefault() // this row accepts the drop
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    if (
      !dnd.over ||
      dnd.over.viewKey !== viewKey ||
      !samePath(dnd.over.path, block.path) ||
      dnd.over.zone !== zone
    )
      dnd.over = { viewKey, path: [...block.path], zone }
  }

  async function drop(e: DragEvent) {
    const from = dnd.from
    if (readonly || !from || from.docName !== docName) return
    e.preventDefault()
    const op = dropMove(from.path, block.path, zoneAt(e))
    endDrag()
    if (op && doc) await doc.apply([op])
  }

  // the logbook: opened per block, remembered on this device
  const logId = $derived(logKey(docName, block.path))
  const logOpen = $derived(isLogOpen(logId))
  const latest = $derived(block.logbook[block.logbook.length - 1])
  const hasChildren = $derived(block.children.length > 0)
  const prog = $derived(progress(block))
  let chipEdit = $state<'scheduled' | 'deadline' | 'priority' | null>(null)

  // The menu of the bullet: copy a reference to the block (`((id))`), an embed (`!((id))`) or its text.
  let menu = $state<{ x: number; y: number } | null>(null)
  function openMenu(e: MouseEvent) {
    e.preventDefault()
    e.stopPropagation()
    menu = { x: e.clientX, y: e.clientY }
  }
  async function copyRef(embed: boolean) {
    const uid = await ensureBlockUid(getDoc(docName), block.id)
    if (!uid) return toast('That block is gone', 'error')
    const text = embed ? `!((${uid}))` : `((${uid}))`
    const ok = await copyText(text)
    toast(ok ? `Copied ${text}` : 'Could not copy', ok ? 'info' : 'error')
  }
  // with several blocks selected, the menu is for the selection
  const multi = $derived(isSelected && editing.selection.length > 1)
  function selectionItems() {
    if (!ctx) return []
    const c = ctx
    return [
      { label: 'Copy', hint: 'Ctrl+C', run: () => void selectionCopy(c) },
      { label: 'Cut', hint: 'Ctrl+X', run: () => void selectionCut(c) },
      { label: 'Duplicate', hint: 'Ctrl+D', run: () => void selectionDuplicate(c) },
      { label: 'Copy block references', hint: 'Ctrl+Shift+C', run: () => void selectionCopyRefs(c) },
      { label: 'Delete', hint: 'Del', run: () => void selectionDelete(c) },
    ]
  }
  function menuItems() {
    if (multi) return selectionItems()
    const items: { label: string; hint?: string; run: () => void }[] = [
      {
        label: 'Copy block reference',
        hint: block.uid ? `((${block.uid}))` : '((id))',
        run: () => void copyRef(false),
      },
      {
        label: 'Copy as embed',
        hint: block.uid ? `!((${block.uid}))` : '!((id))',
        run: () => void copyRef(true),
      },
      {
        label: 'Copy block text',
        run: () =>
          void copyText(block.text).then((ok) =>
            toast(ok ? 'Copied the text' : 'Could not copy', ok ? 'info' : 'error'),
          ),
      },
      {
        label: 'Copy with children',
        hint: 'Markdown',
        run: () =>
          void copyText(toMarkdown([block])).then((ok) =>
            toast(ok ? 'Copied the block' : 'Could not copy', ok ? 'info' : 'error'),
          ),
      },
    ]
    if (!readonly)
      items.push({
        label: 'Zoom in',
        run: () => navigate({ kind: 'page', name: docName, zoom: block.path }),
      })
    return items
  }

  let editor = $state<Editor>() // the open editor of this block, to give it its focus back after a move

  async function applyChip(key: string, value: string | null) {
    chipEdit = null
    if (doc) await setProperty(doc, block.path, key, value)
  }

  function edit(e: MouseEvent) {
    if (readonly) return
    if ((e.target as HTMLElement).closest('a, button, input, .logbook')) return
    e.stopPropagation()
    // text marked with the mouse (to copy it) stays marked: the click that ends the drag must not open the editor,
    // which would replace the text and lose the selection
    const marked = window.getSelection()
    if (
      marked &&
      !marked.isCollapsed &&
      marked.toString().trim() !== '' &&
      (e.currentTarget as Node).contains(marked.anchorNode)
    )
      return
    // Shift+click selects from the block being edited or selected to this one; Ctrl+click adds or removes this one
    if (e.shiftKey && ctx) return selectRangeTo(ctx, block.id)
    if ((e.ctrlKey || e.metaKey) && ctx) return toggleInSelection(ctx, block.id)
    startEditing(viewKey, docName, block.id, cursorFromClick(e))
  }

  /** Maps the click position in the rendered text to an offset in the raw Markdown (best effort). */
  function cursorFromClick(e: MouseEvent): number | 'end' {
    const r = document.caretRangeFromPoint?.(e.clientX, e.clientY)
    if (!r || r.startContainer.nodeType !== Node.TEXT_NODE) return 'end'
    const text = r.startContainer.textContent ?? ''
    const idx = text.length > 0 ? block.text.indexOf(text) : -1
    return idx >= 0 ? idx + r.startOffset : 'end'
  }
</script>

{#snippet details()}
  {#if block.task}
    <div class="chips">
      {#if block.task.scheduled}<button
          class="chip"
          data-chip="scheduled"
          title="Change the scheduled date"
          onclick={() => (chipEdit = chipEdit === 'scheduled' ? null : 'scheduled')}
          ><span class="lab">Scheduled:</span> {block.task.scheduled}</button
        >{/if}
      {#if block.task.deadline}<button
          class="chip"
          class:overdue={isOverdue(block.task.deadline, app.today) && block.task.state !== 'DONE'}
          data-chip="deadline"
          title="Change the deadline"
          onclick={() => (chipEdit = chipEdit === 'deadline' ? null : 'deadline')}
          ><span class="lab">Deadline:</span> {block.task.deadline}</button
        >{/if}

      {#if block.task.chain}<TaskChain page={docName} path={block.path} />{/if}
      {#if block.task.repeat}<span class="chip"><span class="lab">Repeat:</span> {block.task.repeat}</span
        >{/if}
      {#if chipEdit === 'priority'}
        <select
          aria-label="Priority"
          value={block.task.priority ?? ''}
          onchange={(e) => applyChip('priority', (e.currentTarget as HTMLSelectElement).value || null)}
        >
          <option value="">none</option>
          {#each app.graph?.priority_labels ?? [] as lbl (lbl)}
            <option value={lbl}>{lbl}</option>
          {/each}
        </select>
      {:else if chipEdit}
        <div class="chip-picker">
          <DatePicker
            title={chipEdit === 'scheduled' ? 'Scheduled date' : 'Deadline'}
            initial={(chipEdit === 'scheduled' ? block.task.scheduled : block.task.deadline) ?? ''}
            withTime
            onpick={(iso) => applyChip(chipEdit as string, iso)}
            oncancel={() => (chipEdit = null)}
            onclear={() => applyChip(chipEdit as string, null)}
          />
        </div>
      {/if}
    </div>
  {/if}
  {#if block.logbook.length}
    <!-- Logbook: one more labelled row like SCHEDULED/DEADLINE, showing the latest entry; open for the
               whole history, newest first -->
    <div class="logbook" data-testid="logbook" data-open={logOpen}>
      <div class="log-line">
        <button
          class="log-toggle"
          aria-expanded={logOpen}
          aria-controls={`log-${block.id}`}
          title={logOpen ? 'Hide the logbook' : 'Show the whole logbook'}
          onclick={(e) => {
            e.stopPropagation()
            toggleLog(logId)
          }}><span class="chev" aria-hidden="true"></span><span class="lab">Logbook:</span></button
        >
        {#if !logOpen}
          <span class="log-preview"
            ><time>{latest.when}</time>
            <span class="note"><Nodes nodes={latest.content} {chain} /></span></span
          >
          {#if block.logbook.length > 1}<button
              class="log-more"
              onclick={(e) => {
                e.stopPropagation()
                toggleLog(logId)
              }}>+{block.logbook.length - 1} earlier</button
            >{/if}
        {:else}
          <span class="log-count">{block.logbook.length} entr{block.logbook.length === 1 ? 'y' : 'ies'}</span>
        {/if}
      </div>
      {#if logOpen}
        <ol class="timeline" id={`log-${block.id}`} aria-label="Logbook entries, newest first">
          {#each [...block.logbook].reverse() as e, i (i)}
            <li><time>{e.when}</time><span class="note"><Nodes nodes={e.content} {chain} /></span></li>
          {/each}
        </ol>
      {/if}
    </div>
  {/if}
{/snippet}

<div
  class="block"
  class:selected={isSelected}
  class:on-path={onPath}
  class:dragging
  data-block-id={block.id}
  data-doc={readonly ? undefined : docName}
  data-view={viewKey}
  role="treeitem"
  aria-selected={isSelected}
  aria-expanded={hasChildren ? !block.collapsed : undefined}
>
  <div
    class="row"
    class:drop-before={dropZone === 'before'}
    class:drop-after={dropZone === 'after'}
    class:drop-child={dropZone === 'child'}
    ondragover={dragOver}
    ondrop={drop}
    role="presentation"
  >
    {#if hasChildren && !readonly}
      <button
        class="arrow"
        aria-label={block.collapsed ? 'Expand' : 'Collapse'}
        onclick={() => doc?.toggleCollapsed(block.id)}>{block.collapsed ? '▸' : '▾'}</button
      >
    {/if}
    <button
      class="bullet"
      class:collapsed={block.collapsed && hasChildren}
      draggable={!readonly}
      ondragstart={startDrag}
      ondragend={endDrag}
      title={(readonly
        ? 'Right-click for the block menu'
        : 'Click to zoom in, drag to move, right-click for more') + (block.uid ? ` · ^${block.uid}` : '')}
      aria-label="Zoom into block"
      oncontextmenu={openMenu}
      onclick={(e) => {
        if (e.altKey) openMenu(e)
        else if (!readonly) navigate({ kind: 'page', name: docName, zoom: block.path })
      }}>●</button
    >
    {#if menu}
      <BlockMenu
        x={menu.x}
        y={menu.y}
        title={multi
          ? `${editing.selection.length} blocks selected`
          : block.uid
            ? `^${block.uid}`
            : 'no ID yet'}
        items={menuItems()}
        onclose={() => (menu = null)}
      />
    {/if}
    <div class="body" role="presentation" onclick={edit} oncontextmenu={(e) => multi && openMenu(e)}>
      {#if isEditing && ctx && editing.conflict && editing.conflict.blockId === block.id}
        <div class="conflict" role="alert" data-testid="block-conflict">
          <strong>This block changed on disk while you were editing it.</strong>
          <div class="versions">
            <div>
              <em>Yours</em>
              <pre>{editing.conflict.mine}</pre>
            </div>
            <div>
              <em>On disk</em>
              <pre>{editing.conflict.theirs}</pre>
            </div>
          </div>
          <button onclick={() => keepMine(ctx, block.id)}>Keep mine</button>
          <button onclick={() => takeTheirs()}>Take theirs</button>
        </div>
      {/if}
      {#if isEditing && ctx}
        {#key editing.nonce}
          <Editor
            bind:this={editor}
            text={block.text}
            cursor={editing.cursor}
            pageName={docName}
            onchange={(t, prev) => commitText(ctx, block.id, t, prev, { onlyIfActive: true })}
            onaction={(a) => editorAction(ctx, block.id, a)}
            onundo={(k) => globalUndo(k)}
            onpaste={(t) => pasteBlocks(ctx, block.id, t)}
            onblur={async (t, prev) => {
              // only the editor that is still the active one may save; a late blur from an editor that was
              // replaced (indent, split, ...) must not write through a stale block id
              if (editing.mode !== 'edit' || editing.blockId !== block.id || editing.viewKey !== viewKey)
                return
              // the editor lost its focus because a move re-parented it a moment ago: bring it back
              if (editing.planBlock === block.id && Date.now() - editing.planAt < 400) {
                editor?.focus()
                return
              }
              await commitText(ctx, block.id, t, prev, { onlyIfActive: true })
              if (editing.mode === 'edit' && editing.blockId === block.id && !editing.conflict) stopEditing()
            }}
          />
        {/key}
      {:else}
        <div
          class="content"
          class:done={block.task?.state === 'DONE' || block.task?.state === 'CANCELED'}
          class:canceled={block.task?.state === 'CANCELED'}
        >
          {#if block.task}<button
              class="marker marker-{block.task.state.toLowerCase()}"
              data-task={block.task.state}
              title="Click to change the task state"
              onclick={(e) => {
                e.stopPropagation()
                if (ctx) void setTaskState(ctx, block.id, nextTaskState(block.task?.state))
              }}>{block.task.state}</button
            >{/if}
          {#if block.task?.priority}
            <button
              class="prio"
              data-chip="priority"
              title="Change the priority"
              onclick={(e) => {
                e.stopPropagation()
                chipEdit = chipEdit === 'priority' ? null : 'priority'
              }}>[#{block.task.priority}]</button
            >{/if}
          {#if block.task && prog.total > 0}<span
              class="progress"
              title="{prog.done} of {prog.total} subtasks done"
              data-testid="progress"
              ><span class="bar"
                ><span class="fill" style:width="{(100 * prog.done) / prog.total}%"></span></span
              >{prog.done}/{prog.total}</span
            >{/if}
          <div class="nodes"><Nodes nodes={block.content} {chain} /></div>
          {#if isCompact}
            <!-- compact task details: small indicators; the full chips and logbook open in a popover -->
            <!-- hover and focus open the popover; the controls inside it are real buttons and inputs -->
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <span
              class="meta"
              role="group"
              aria-label="Task details"
              data-testid="task-meta"
              onmouseenter={() => (detailsOpen = true)}
              onmouseleave={(e) => {
                // an input inside the popover (editing a property) keeps it open
                const a = document.activeElement
                if (!(a instanceof HTMLInputElement && (e.currentTarget as HTMLElement).contains(a)))
                  closeDetails()
              }}
              onfocusin={() => (detailsOpen = true)}
              onfocusout={(e) => {
                if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node | null)) {
                  pinned = false
                  closeDetails()
                }
              }}
              onkeydown={(e) => {
                if (e.key === 'Escape' && detailsOpen) {
                  e.stopPropagation()
                  pinned = false
                  chipEdit = null
                  detailsOpen = false
                }
              }}
            >
              {#if propsCompact}
                <button
                  class="ind"
                  data-indicator="props"
                  title={shownProps(block)
                    .map((p) => `${p.key}: ${p.raw}`)
                    .join('\n')}
                  aria-expanded={detailsOpen}
                  onclick={(e) => {
                    e.stopPropagation()
                    pinned = !pinned
                    detailsOpen = pinned || detailsOpen
                  }}><span aria-hidden="true">⋯</span> {propCount}</button
                >
              {/if}
              {#if due && taskCompact}
                <button
                  class="ind due-{due.tone}"
                  data-indicator="date"
                  title={`${due.kind === 'deadline' ? 'Deadline' : 'Scheduled'} ${due.date}`}
                  aria-expanded={detailsOpen}
                  onclick={(e) => {
                    e.stopPropagation()
                    pinned = !pinned
                    detailsOpen = pinned || detailsOpen
                  }}
                  ><span aria-hidden="true">{due.kind === 'deadline' ? '📅' : '⏳'}</span>
                  {due.text ?? shownDate(due.date.slice(0, 10))}{#if timeLabel(due.date)}
                    <span class="at">{timeLabel(due.date)}</span>{/if}</button
                >
              {/if}
              {#if block.task?.chain && taskCompact}<TaskChain
                  page={docName}
                  path={block.path}
                  compact
                />{/if}
              {#if block.task?.repeat && taskCompact}
                <button
                  class="ind"
                  data-indicator="repeat"
                  title={`Repeats ${block.task.repeat}`}
                  aria-expanded={detailsOpen}
                  onclick={(e) => {
                    e.stopPropagation()
                    pinned = !pinned
                    detailsOpen = pinned || detailsOpen
                  }}><span aria-hidden="true">↻</span></button
                >
              {/if}
              {#if block.logbook.length && taskCompact}
                <button
                  class="ind"
                  data-indicator="logbook"
                  title={`Logbook: ${block.logbook.length} entr${block.logbook.length === 1 ? 'y' : 'ies'}`}
                  aria-expanded={detailsOpen}
                  onclick={(e) => {
                    e.stopPropagation()
                    pinned = !pinned
                    detailsOpen = pinned || detailsOpen
                  }}><span aria-hidden="true">✎</span> {block.logbook.length}</button
                >
              {/if}
              {#if detailsOpen}
                <div
                  class="popover"
                  data-testid="task-details"
                  role="presentation"
                  onclick={(e) => e.stopPropagation()}
                >
                  {#if taskCompact}{@render details()}{/if}
                  {#if propsCompact}<PropertyCard {block} {docName} {readonly} />{/if}
                </div>
              {/if}
            </span>
          {/if}
        </div>
        {#if !taskCompact}{@render details()}{/if}
        {#if !propsCompact}<PropertyCard {block} {docName} {readonly} />{/if}
      {/if}
    </div>
  </div>
  {#if hasChildren && !block.collapsed}
    <div class="children" role="group">
      {#each block.children as child (child.id)}
        <Self block={child} {docName} {viewKey} {readonly} {chain} {compact} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .block {
    position: relative;
  }
  .block.selected > .row {
    background: var(--selection);
    border-radius: 4px;
  }
  /* path to the focused block: the guide line turns dark down to the block's bullet, then an elbow */
  /* `:global` for the child: it is rendered by `<Self>`, which Svelte can't see as a direct child of
     `.children`, and it would drop the rule */
  .children > :global(.block.on-path)::before {
    content: '';
    position: absolute;
    left: -21px;
    top: 0;
    width: 21px;
    height: 0.8em;
    border-left: 2px solid var(--path);
    border-bottom: 2px solid var(--path);
    border-bottom-left-radius: 8px;
    pointer-events: none;
  }
  /* the dark guide also covers the siblings above the path block (`:global`: Svelte can't see the class
     through `~` and would drop the rule) */
  .children > :global(.block:has(~ .on-path))::after {
    content: '';
    position: absolute;
    left: -21px;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--path);
    pointer-events: none;
  }
  /* the stem: from the bullet of a block on the path down to where its children start */
  :global(.block:has(> .children > .on-path) > .row::after) {
    content: '';
    position: absolute;
    left: 8px;
    top: 0.8em;
    bottom: 0;
    width: 2px;
    background: var(--path);
    pointer-events: none;
  }
  .block.dragging > .row {
    opacity: 0.4;
  }
  /* the drop line: above the row, below it, or below it and indented (a child) */
  .row::before {
    content: '';
    position: absolute;
    height: 2px;
    background: var(--accent);
    border-radius: 1px;
    left: 0;
    right: 0;
    opacity: 0;
    pointer-events: none;
  }
  .row.drop-before::before {
    top: -1px;
    opacity: 1;
  }
  .row.drop-after::before {
    bottom: -1px;
    opacity: 1;
  }
  .row.drop-child::before {
    bottom: -1px;
    left: 28px;
    opacity: 1;
  }
  .row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 2px;
  }
  /* the expand arrow sits in the left gutter so the bullet is at the block's left edge, and the guide
     line (which runs under the parent's bullet) lines up with it */
  .arrow {
    position: absolute;
    left: -17px;
    top: 0;
    width: 16px;
    flex: none;
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0;
    line-height: 1.7;
    font-size: 0.8em;
    opacity: 0;
  }
  .block:hover > .row > .arrow,
  .arrow:focus-visible {
    opacity: 1;
  }
  .bullet {
    flex: none;
    width: 18px;
    /* buttons do not inherit the font by default: without this 1.6em is of the browser's 13px, not of the text,
       and the dot sits off the middle of the first line */
    font: inherit;
    height: 1.6em;
    /* flex, not grid: a grid's single row is only as tall as the dot and sits at the top of the button */
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: none;
    /* the glyph in the markup stays as the fallback text; the dot is drawn by ::before */
    color: transparent;
    --dot: var(--muted);
    cursor: pointer;
    padding: 0;
  }
  .bullet::before {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--dot);
    opacity: 0.4;
    transition:
      transform 0.12s,
      opacity 0.12s;
  }
  .bullet:hover::before {
    opacity: 1;
    transform: scale(1.35);
  }
  .bullet.collapsed {
    --dot: var(--accent);
  }
  .bullet.collapsed::before {
    width: 8px;
    height: 8px;
    opacity: 1;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .body {
    flex: 1;
    min-width: 0;
    min-height: 1.6em;
    cursor: text;
    line-height: 1.6;
  }
  .content {
    display: flex;
    gap: 0.4em;
    align-items: baseline;
  }
  .content.done .nodes {
    text-decoration: line-through;
    color: var(--muted);
  }
  .content.canceled .nodes {
    opacity: 0.6;
  }
  /* compact task details: indicators at the end of the first row, the details in a popover under them */
  .meta {
    position: relative;
    flex: none;
    display: inline-flex;
    gap: 0.5em;
    font-size: 0.78em;
    color: var(--muted);
  }
  .ind {
    border: none;
    background: none;
    padding: 0;
    cursor: pointer;
    color: inherit;
    font: inherit;
    white-space: nowrap;
  }
  .ind:hover,
  .ind[aria-expanded='true'] {
    color: var(--text);
  }
  .ind.due-overdue {
    color: var(--danger);
    font-weight: 600;
  }
  .ind.due-today {
    color: var(--accent);
    font-weight: 600;
  }
  .popover {
    position: absolute;
    z-index: 30;
    top: 100%;
    right: 0;
    /* an absolutely positioned box shrinks to its (small) container: ask for the content's width instead, or
       an open logbook is squeezed into a tall narrow column */
    width: max-content;
    min-width: 18em;
    max-width: min(40em, 85vw);
    padding: 0.4em 0.7em;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.2);
    font-size: 1.1em;
    cursor: default;
  }
  .nodes {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  /* Logseq-like task marker: a teal dot and the state in a soft pill; a finished task gets a tick */
  .marker {
    display: inline-flex;
    align-items: center;
    gap: 0.35em;
    border: none;
    cursor: pointer;
    font-size: 0.72em;
    font-weight: 700;
    letter-spacing: 0.04em;
    padding: 0.1em 0.65em 0.1em 0.3em;
    border-radius: 999px;
    background: var(--chip);
    color: var(--text);
  }
  .marker::before {
    content: '';
    flex: none;
    width: 1.25em;
    height: 1.25em;
    border-radius: 50%;
    background: var(--task);
  }
  .marker-done::before {
    background:
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='none' stroke='white' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M4 8.4l2.6 2.6L12 5.6'/%3E%3C/svg%3E")
        center / 80% no-repeat,
      var(--task);
  }
  .marker-doing {
    background: color-mix(in srgb, var(--task) 22%, var(--chip));
  }
  .marker-waiting::before {
    background: transparent;
    box-shadow: inset 0 0 0 2px var(--task);
  }
  .marker-canceled {
    color: var(--muted);
  }
  .marker-canceled::before {
    background: var(--muted);
    opacity: 0.5;
  }
  /* "SCHEDULED: 2026-10-10" lines under a task, like Logseq's */
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0 0.8em;
    font-size: 0.78em;
    color: var(--muted);
  }
  .chips {
    flex-direction: column;
    align-items: flex-start;
    gap: 0;
    line-height: 1.55;
  }
  .chip-picker {
    flex-basis: 100%;
  }
  .chip {
    border: none;
    background: none;
    padding: 0;
    cursor: pointer;
    color: var(--link);
    font: inherit;
    text-align: left;
  }
  .chip .lab {
    color: var(--muted);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .chip:hover {
    text-decoration: underline;
  }
  .chip.overdue {
    color: var(--danger);
    font-weight: 600;
  }
  /* [#A] after the task marker, as Logseq writes it */
  .prio {
    border: none;
    background: none;
    padding: 0;
    cursor: pointer;
    color: var(--link);
    font-size: 0.85em;
    font-weight: 600;
    align-self: baseline;
  }
  .prio:hover {
    text-decoration: underline;
  }
  .progress {
    font-size: 0.75em;
    color: var(--muted);
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
  }
  .bar {
    width: 40px;
    height: 5px;
    background: var(--chip);
    border-radius: 3px;
    overflow: hidden;
    display: inline-block;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  /* the logbook row: same label style as SCHEDULED/DEADLINE; the timeline is the history, newest first */
  .logbook {
    font-size: 0.78em;
    color: var(--muted);
    margin: 0;
    line-height: 1.55;
  }
  .log-line {
    display: flex;
    align-items: baseline;
    gap: 0.5em;
    min-width: 0;
  }
  .log-toggle,
  .log-more {
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .log-toggle {
    display: inline-flex;
    align-items: baseline;
    gap: 0.35em;
    flex: none;
  }
  .log-toggle .lab {
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .log-toggle:hover .lab,
  .log-more:hover {
    color: var(--text);
  }
  /* a small chevron instead of the browser's disclosure triangle */
  .chev {
    display: inline-block;
    width: 0.42em;
    height: 0.42em;
    border-right: 1.6px solid currentColor;
    border-bottom: 1.6px solid currentColor;
    transform: translateY(-0.12em) rotate(-45deg);
    transition: transform 0.12s;
  }
  .logbook[data-open='true'] .chev {
    transform: translateY(-0.2em) rotate(45deg);
  }
  .log-preview {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .log-preview :global(p),
  .timeline :global(p) {
    display: inline;
    margin: 0;
  }
  .log-more {
    flex: none;
    color: var(--link);
  }
  .log-more:hover {
    text-decoration: underline;
  }
  .log-count {
    color: var(--muted);
  }
  .logbook time {
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    white-space: nowrap;
  }
  .timeline {
    list-style: none;
    margin: 0.2em 0 0.3em 0.2em;
    padding: 0 0 0 0.9em;
    border-left: 2px solid var(--border);
  }
  .timeline li {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0 1em;
    padding: 0.1em 0;
    color: var(--text);
  }
  .timeline time {
    font-size: 0.95em;
  }
  @media (prefers-reduced-motion: reduce) {
    .chev {
      transition: none;
    }
  }
  .conflict {
    border: 1px solid var(--warn);
    background: var(--warn-bg);
    border-radius: 6px;
    padding: 0.4em 0.6em;
    margin-bottom: 0.3em;
    font-size: 0.85em;
  }
  .versions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5em;
  }
  .versions pre {
    margin: 0.1em 0;
    white-space: pre-wrap;
    font-family: var(--font-mono);
  }
  .children {
    margin-left: 8px;
    padding-left: 20px;
    border-left: 1px solid var(--border);
  }
  .ind .at {
    margin-left: 0.25em;
    font-variant-numeric: tabular-nums;
  }
</style>
