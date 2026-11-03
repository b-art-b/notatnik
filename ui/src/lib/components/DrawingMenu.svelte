<script lang="ts">
  // The "More" menu of the drawing editor: a button that opens a list above it. Items are plain, a choice among
  // several (radio) or an option that is on or off (check). It closes on a choice, on a press outside and on Escape;
  // the Escape is handled here and marked as handled, so the dialog around it does not take it as Cancel.
  export type MenuItem =
    | { kind: 'heading'; label: string }
    | { kind: 'sep' }
    | {
        kind: 'item' | 'radio' | 'check'
        label: string
        checked?: boolean
        disabled?: boolean
        testid?: string
        onselect: () => void
      }

  let { items, onopen, label = 'More' }: { items: MenuItem[]; onopen?: () => void; label?: string } = $props()

  let open = $state(false)
  let root: HTMLDivElement | undefined = $state()
  let list: HTMLDivElement | undefined = $state()

  function toggle() {
    open = !open
    if (open) {
      onopen?.()
      // the first enabled item gets the focus, so the keyboard can go on from there
      queueMicrotask(() => list?.querySelector<HTMLElement>('[role^="menuitem"]:not(:disabled)')?.focus())
    }
  }

  function choose(it: Extract<MenuItem, { onselect: () => void }>) {
    open = false
    it.onselect()
  }

  function key(e: KeyboardEvent) {
    if (!open) return
    if (e.key === 'Escape') {
      e.preventDefault()
      open = false
      root?.querySelector<HTMLElement>('button.trigger')?.focus()
      return
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const all = [...(list?.querySelectorAll<HTMLElement>('[role^="menuitem"]:not(:disabled)') ?? [])]
      if (all.length === 0) return
      const at = all.indexOf(document.activeElement as HTMLElement)
      const next = e.key === 'ArrowDown' ? (at + 1) % all.length : (at - 1 + all.length) % all.length
      all[next].focus()
    }
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (open && root && !root.contains(e.target as Node)) open = false
  }}
/>

<div class="menu" bind:this={root} onkeydown={key} role="presentation">
  <button
    type="button"
    class="trigger"
    aria-haspopup="menu"
    aria-expanded={open}
    data-testid="drawing-more"
    onclick={toggle}>{label} <span aria-hidden="true">▾</span></button
  >
  {#if open}
    <div class="list" role="menu" bind:this={list} aria-label={label} data-testid="drawing-more-menu">
      {#each items as it, i (i)}
        {#if it.kind === 'heading'}
          <div class="heading" role="presentation">{it.label}</div>
        {:else if it.kind === 'sep'}
          <hr />
        {:else}
          <button
            type="button"
            role={it.kind === 'radio'
              ? 'menuitemradio'
              : it.kind === 'check'
                ? 'menuitemcheckbox'
                : 'menuitem'}
            aria-checked={it.kind === 'item' ? undefined : !!it.checked}
            disabled={it.disabled}
            data-testid={it.testid}
            onclick={() => choose(it)}
          >
            <span class="mark" aria-hidden="true">{it.kind === 'item' ? '' : it.checked ? '✓' : ''}</span>
            {it.label}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }
  .trigger {
    font: inherit;
    padding: 0.25em 0.7em;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .list {
    position: absolute;
    right: 0;
    bottom: calc(100% + 6px);
    min-width: 17em;
    max-height: 70vh;
    overflow-y: auto;
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.25);
    z-index: 80;
  }
  .list button {
    display: flex;
    align-items: center;
    gap: 0.4em;
    width: 100%;
    font: inherit;
    text-align: left;
    padding: 0.4em 0.6em;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .list button:hover:not(:disabled),
  .list button:focus-visible {
    background: var(--chip);
    outline: none;
  }
  .list button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .mark {
    width: 1.1em;
    flex: none;
  }
  .heading {
    padding: 0.4em 0.6em 0.1em;
    font-size: 0.75em;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }
  @media (pointer: coarse) {
    .list button {
      min-height: 44px;
    }
    .trigger {
      min-height: 44px;
    }
  }
</style>
