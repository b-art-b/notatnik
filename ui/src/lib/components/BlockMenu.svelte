<script lang="ts">
  // The menu of a block's bullet (right-click, or Alt+click): copy a reference to the block, copy it as an embed,
  // copy its text, zoom in. A reference is `((id))`; pasted into any block it is a link to this block.
  import { onMount } from 'svelte'

  interface Item {
    label: string
    hint?: string
    run: () => void
  }
  let {
    x,
    y,
    title = '',
    items,
    onclose,
  }: { x: number; y: number; title?: string; items: Item[]; onclose: () => void } = $props()
  let menu: HTMLDivElement
  let at = $state({ x: 0, y: 0 })

  onMount(() => {
    // keep the menu inside the window
    const r = menu.getBoundingClientRect()
    at = {
      x: Math.max(4, Math.min(x, window.innerWidth - r.width - 4)),
      y: Math.max(4, Math.min(y, window.innerHeight - r.height - 4)),
    }
    menu.querySelector<HTMLButtonElement>('button')?.focus()
    const away = (e: Event) => {
      if (!menu.contains(e.target as Node)) onclose()
    }
    // `pointerdown` in the capture phase: a click anywhere else closes it before that click does anything
    window.addEventListener('pointerdown', away, true)
    window.addEventListener('blur', onclose)
    return () => {
      window.removeEventListener('pointerdown', away, true)
      window.removeEventListener('blur', onclose)
    }
  })

  function key(e: KeyboardEvent) {
    e.stopPropagation()
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>('button')]
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement)
    if (e.key === 'Escape') {
      e.preventDefault()
      onclose()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const n = buttons.length
      buttons[(i + (e.key === 'ArrowDown' ? 1 : n - 1)) % n]?.focus()
    }
  }
</script>

<!-- a click inside must not reach the block's own click handler (it would open the editor) -->
<div
  bind:this={menu}
  class="menu"
  role="menu"
  aria-label="Block"
  tabindex="-1"
  data-testid="block-menu"
  style:left={`${at.x}px`}
  style:top={`${at.y}px`}
  onkeydown={key}
  onclick={(e) => e.stopPropagation()}
>
  {#if title}<div class="title" role="none" data-testid="block-menu-id">{title}</div>{/if}
  {#each items as it (it.label)}
    <button
      type="button"
      role="menuitem"
      onclick={() => {
        onclose()
        it.run()
      }}
      >{it.label}{#if it.hint}<span class="hint">{it.hint}</span>{/if}</button
    >
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 50;
    min-width: 14em;
    padding: 0.25em;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.2);
    display: flex;
    flex-direction: column;
  }
  button {
    display: flex;
    justify-content: space-between;
    gap: 1.5em;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    padding: 0.35em 0.6em;
    border-radius: 5px;
    cursor: pointer;
    font: inherit;
  }
  button:hover,
  button:focus-visible {
    background: var(--chip);
    outline: none;
  }
  .title {
    padding: 0.2em 0.6em 0.3em;
    color: var(--muted);
    font-size: 0.8em;
    font-family: var(--font-mono);
    border-bottom: 1px solid var(--border);
    margin-bottom: 0.2em;
  }
  .hint {
    color: var(--muted);
    font-size: 0.85em;
  }
</style>
