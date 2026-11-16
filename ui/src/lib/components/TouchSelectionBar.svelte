<script lang="ts">
  // (Not the SelectionBar of the PDF reader.) A touch screen has no Shift or Ctrl and no right button, so selected blocks get a bar at the bottom of the screen
  // with the commands that matter on a phone: move them up and down and out and in (the Alt+arrows and Tab keys), make them
  // tasks, copy, delete, and Done to let go of the selection.
  // Blocks are added to the selection by tapping them (`Block.svelte`, `edit`) after "Select" in a bullet's menu.
  import { editing, stopEditing } from '../stores/editing.svelte'
  import { getDoc } from '../stores/pages.svelte'
  import { isMobile } from '../platform'
  import {
    selectionCopy,
    selectionDelete,
    selectionIndent,
    selectionMakeTasks,
    selectionMove,
    selectionOutdent,
  } from '../commands/exec'

  // a finger as the main pointer; `isMobile` is the phone app itself
  const touch = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
  const show = $derived((isMobile || touch) && editing.mode === 'select' && editing.selection.length > 0)
  const ctx = () => ({ doc: getDoc(editing.docName), viewKey: editing.viewKey })
</script>

{#if show}
  <div class="bar" role="toolbar" aria-label="Selected blocks" data-testid="touch-selection-bar">
    <div class="row">
      <span class="count">{editing.selection.length} selected</span>
      <button type="button" aria-label="Move up" onclick={() => void selectionMove(ctx(), 'up')}>↑</button>
      <button type="button" aria-label="Move down" onclick={() => void selectionMove(ctx(), 'down')}>↓</button
      >
      <button type="button" aria-label="Outdent" onclick={() => void selectionOutdent(ctx())}>←</button>
      <button type="button" aria-label="Indent" onclick={() => void selectionIndent(ctx())}>→</button>
    </div>
    <div class="row">
      <button type="button" onclick={() => void selectionMakeTasks(ctx())}>Tasks</button>
      <button type="button" onclick={() => void selectionCopy(ctx())}>Copy</button>
      <button type="button" class="danger" onclick={() => void selectionDelete(ctx())}>Delete</button>
      <button type="button" class="done" onclick={() => stopEditing()}>Done</button>
    </div>
  </div>
{/if}

<style>
  .bar {
    position: fixed;
    inset: auto 0 0 0;
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 0.4em;
    padding: 0.5em 0.7em;
    background: var(--panel, var(--bg));
    border-top: 1px solid var(--border, #8884);
    box-shadow: 0 -2px 8px #0003;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.4em;
  }
  .row button {
    flex: 1;
  }
  .count {
    flex: 1;
    min-width: 0;
    color: var(--muted);
    font-size: 0.9em;
    white-space: nowrap;
  }
  button {
    min-height: 2.6em;
    padding: 0 0.9em;
    border: 1px solid var(--border, #8886);
    border-radius: 8px;
    background: var(--chip, transparent);
    color: var(--text);
    font: inherit;
  }
  .danger {
    color: var(--task-overdue, #c33);
  }
  .done {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text, #fff);
  }
</style>
