<script lang="ts">
  import type { BlockView } from '../api'
  import { editing, startEditing, startSelecting, stopEditing } from '../stores/editing.svelte'
  import { getDoc } from '../stores/pages.svelte'
  import { navigate } from '../stores/app.svelte'
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
  } from '../commands/exec'
  import { app } from '../stores/app.svelte'
  import { isOverdue, nextTaskState, progress } from '../tasks'
  import Nodes from './Nodes.svelte'
  import Editor from './Editor.svelte'
  import Self from './Block.svelte'
  import { pageHash } from '../route'

  interface Props {
    block: BlockView
    docName: string
    viewKey: string
    readonly?: boolean
    chain?: string[]
  }
  let { block, docName, viewKey, readonly = false, chain = [] }: Props = $props()

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
  const visibleProps = $derived(block.props.filter((p) => !p.hidden))
  const hasChildren = $derived(block.children.length > 0)
  const prog = $derived(progress(block))
  let chipEdit = $state<'scheduled' | 'deadline' | 'priority' | null>(null)

  async function applyChip(key: string, value: string | null) {
    chipEdit = null
    if (doc) await setProperty(doc, block.path, key, value)
  }

  function edit(e: MouseEvent) {
    if (readonly) return
    if ((e.target as HTMLElement).closest('a, button, input')) return
    e.stopPropagation()
    if (e.shiftKey) {
      startSelecting(viewKey, docName, [block.id])
      return
    }
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

<div
  class="block"
  class:selected={isSelected}
  data-block-id={block.id}
  role="treeitem"
  aria-selected={isSelected}
  aria-expanded={hasChildren ? !block.collapsed : undefined}
>
  <div class="row">
    {#if hasChildren && !readonly}
      <button
        class="arrow"
        aria-label={block.collapsed ? 'Expand' : 'Collapse'}
        onclick={() => doc?.toggleCollapsed(block.id)}>{block.collapsed ? '▸' : '▾'}</button
      >
    {:else}
      <span class="arrow spacer"></span>
    {/if}
    <button
      class="bullet"
      class:collapsed={block.collapsed && hasChildren}
      aria-label="Zoom into block"
      onclick={() => !readonly && navigate({ kind: 'page', name: docName, zoom: block.path })}>●</button
    >
    <div class="body" role="presentation" onclick={edit}>
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
          {#if prog.total > 0}<span
              class="progress"
              title="{prog.done} of {prog.total} subtasks done"
              data-testid="progress"
              ><span class="bar"
                ><span class="fill" style:width="{(100 * prog.done) / prog.total}%"></span></span
              >{prog.done}/{prog.total}</span
            >{/if}
          <div class="nodes"><Nodes nodes={block.content} {chain} /></div>
        </div>
        {#if block.task}
          <div class="chips">
            {#if block.task.scheduled}<button
                class="chip"
                data-chip="scheduled"
                onclick={() => (chipEdit = chipEdit === 'scheduled' ? null : 'scheduled')}
                >⏳ {block.task.scheduled}</button
              >{/if}
            {#if block.task.deadline}<button
                class="chip"
                class:overdue={isOverdue(block.task.deadline, app.today) && block.task.state !== 'DONE'}
                data-chip="deadline"
                onclick={() => (chipEdit = chipEdit === 'deadline' ? null : 'deadline')}
                >📅 {block.task.deadline}</button
              >{/if}
            {#if block.task.priority}<button
                class="chip prio-{block.task.priority}"
                data-chip="priority"
                onclick={() => (chipEdit = chipEdit === 'priority' ? null : 'priority')}
                >{block.task.priority}</button
              >{/if}
            {#if block.task.repeat}<span class="chip">🔁 {block.task.repeat}</span>{/if}
            {#if chipEdit === 'priority'}
              <select
                aria-label="Priority"
                value={block.task.priority ?? ''}
                onchange={(e) => applyChip('priority', (e.currentTarget as HTMLSelectElement).value || null)}
              >
                <option value="">none</option><option value="high">high</option><option value="medium"
                  >medium</option
                ><option value="low">low</option>
              </select>
            {:else if chipEdit}
              <input
                type="date"
                aria-label={chipEdit}
                value={(chipEdit === 'scheduled' ? block.task.scheduled : block.task.deadline)?.slice(
                  0,
                  10,
                ) ?? ''}
                onchange={(e) =>
                  applyChip(chipEdit as string, (e.currentTarget as HTMLInputElement).value || null)}
              />
              <button class="chip" onclick={() => applyChip(chipEdit as string, null)}>clear</button>
            {/if}
          </div>
        {/if}
        {#if visibleProps.length}
          <div class="props">
            {#each visibleProps as p (p.key)}
              {#if !(block.task && ['scheduled', 'deadline', 'priority', 'repeat'].includes(p.key))}
                <span class="prop"
                  ><span class="prop-key">{p.key}:</span>
                  {#if p.links.length}{#each p.links as l, i (i)}<a class="page-ref" href={pageHash(l)}>{l}</a
                      >{#if i < p.links.length - 1},
                      {/if}{/each}{:else}{p.raw}{/if}</span
                >
              {/if}
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  </div>
  {#if hasChildren && !block.collapsed}
    <div class="children" role="group">
      {#each block.children as child (child.id)}
        <Self block={child} {docName} {viewKey} {readonly} {chain} />
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
  .row {
    display: flex;
    align-items: flex-start;
    gap: 2px;
  }
  .arrow {
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
  .spacer {
    display: inline-block;
    cursor: default;
  }
  .bullet {
    flex: none;
    width: 18px;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 0.55em;
    line-height: 3.1;
    cursor: pointer;
    padding: 0;
  }
  .bullet.collapsed {
    color: var(--accent);
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
  .nodes {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .marker {
    border: none;
    cursor: pointer;
    font-size: 0.7em;
    font-weight: 700;
    padding: 0 0.4em;
    border-radius: 4px;
    background: var(--chip);
    color: var(--muted);
  }
  .marker-doing {
    background: var(--accent);
    color: white;
  }
  .marker-todo {
    color: var(--text);
  }
  .chips,
  .props {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3em;
    font-size: 0.78em;
    color: var(--muted);
  }
  .chip {
    border: none;
    cursor: pointer;
    color: inherit;
  }
  .chip.overdue {
    background: var(--danger);
    color: white;
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
  .prop-key {
    opacity: 0.8;
  }
  .children {
    margin-left: 12px;
    padding-left: 10px;
    border-left: 1px solid var(--border);
  }
</style>
