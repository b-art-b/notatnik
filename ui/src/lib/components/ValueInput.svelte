<script lang="ts">
  // A value field for a property: suggests the values already used for it and pages. Picking a page writes
  // `[[Page]]`. Used for page properties and for the card under a block.
  //
  // Svelte idioms here: `$bindable` makes `value` usable with `bind:value` from the parent; `$derived` computes the
  // suggestions from what is typed; the list closes on blur, but a click on it is handled on `mousedown` (which
  // comes before the blur) so the pick is not lost.
  import { onMount } from 'svelte'
  import { backend, type TitleHit } from '../api'

  interface Props {
    value: string
    /** The accessible name of the field. */
    label: string
    placeholder?: string
    /** Values already used for this property (shown first). */
    values?: string[]
    /** Called once, when the field is first focused, if the values are not known yet. */
    loadValues?: () => Promise<string[]>
    /** Called with the final value: a pick, Enter or leaving the field after an edit. */
    oncommit?: (value: string) => void
    onescape?: () => void
    /** Focus and select the text when the field appears. */
    focusOnMount?: boolean
    /** How a picked page is written: `[[Page]]` (a property value) or the plain name (a filter field). */
    pageFormat?: 'link' | 'name'
  }
  let {
    value = $bindable(''),
    label,
    placeholder = '',
    values = [],
    loadValues,
    oncommit,
    onescape,
    focusOnMount = false,
    pageFormat = 'link',
  }: Props = $props()

  let input: HTMLInputElement
  onMount(() => {
    if (focusOnMount) {
      input.focus()
      input.select()
    }
  })

  let open = $state(false)
  let active = $state(-1)
  let pages = $state<TitleHit[]>([])
  let loaded = $state<string[]>([])
  let loadedOnce = false
  let lastCommitted: string | null = null
  const id = `vi-${Math.random().toString(36).slice(2, 8)}`

  /** What is searched: the text typed, without the brackets of a link being written. */
  const needle = $derived(value.trim().replace(/^\[\[/, '').replace(/\]\]$/, '').toLowerCase())
  const used = $derived(
    [...new Set([...values, ...loaded])]
      .filter((v) => v !== value && (!needle || v.toLowerCase().includes(needle)))
      .slice(0, 8),
  )
  const items = $derived([
    ...used.map((text) => ({ kind: 'value' as const, text })),
    ...pages.map((p) => ({ kind: 'page' as const, text: p.name })),
  ])

  // pages matching what is typed (a short delay, so typing does not search on every key)
  $effect(() => {
    const q = needle
    if (!open || !q) {
      pages = []
      return
    }
    let cancelled = false
    const t = setTimeout(async () => {
      try {
        const hits = await backend.call('search_titles', { text: q, limit: 8 })
        if (!cancelled) pages = hits
      } catch {
        if (!cancelled) pages = []
      }
    }, 120)
    return () => {
      cancelled = true
      clearTimeout(t)
    }
  })

  async function onfocus() {
    open = true
    if (loadValues && !loadedOnce) {
      loadedOnce = true
      try {
        loaded = await loadValues()
      } catch {
        loaded = []
      }
    }
  }

  function commit(v: string) {
    if (v === lastCommitted) return
    lastCommitted = v
    oncommit?.(v)
  }

  function pick(i: number) {
    const it = items[i]
    if (!it) return
    value = it.kind === 'page' && pageFormat === 'link' ? `[[${it.text}]]` : it.text
    open = false
    active = -1
    commit(value)
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' && items.length) {
      e.preventDefault()
      open = true
      active = (active + 1) % items.length
    } else if (e.key === 'ArrowUp' && items.length) {
      e.preventDefault()
      active = (active - 1 + items.length) % items.length
    } else if (e.key === 'Enter' && open && active >= 0) {
      e.preventDefault()
      e.stopPropagation()
      pick(active)
    } else if (e.key === 'Escape') {
      e.stopPropagation()
      if (open && items.length) open = false
      else onescape?.()
    }
  }
</script>

<span class="vi">
  <input
    bind:this={input}
    bind:value
    role="combobox"
    aria-label={label}
    aria-expanded={open && items.length > 0}
    aria-controls={`${id}-list`}
    aria-autocomplete="list"
    aria-activedescendant={active >= 0 ? `${id}-${active}` : undefined}
    autocomplete="off"
    {placeholder}
    oninput={() => {
      open = true
      active = -1
    }}
    {onfocus}
    onblur={() => (open = false)}
    onchange={() => commit(value)}
    {onkeydown}
  />
  {#if open && items.length}
    <ul class="list" id={`${id}-list`} role="listbox" aria-label="Suggestions">
      {#each items as it, i (it.kind + it.text)}
        <!-- mousedown, not click: it comes before the input's blur closes the list -->
        <li
          id={`${id}-${i}`}
          role="option"
          aria-selected={i === active}
          class:on={i === active}
          data-kind={it.kind}
          onmousedown={(e) => {
            e.preventDefault()
            pick(i)
          }}
        >
          <span class="text">{it.text}</span><span class="tag">{it.kind}</span>
        </li>
      {/each}
    </ul>
  {/if}
</span>

<style>
  .vi {
    position: relative;
    display: inline-block;
  }
  .list {
    position: absolute;
    z-index: 30;
    left: 0;
    top: 100%;
    margin: 2px 0 0;
    padding: 0.2em;
    min-width: 100%;
    max-width: 28em;
    max-height: 14em;
    overflow: auto;
    list-style: none;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.18);
  }
  li {
    display: flex;
    justify-content: space-between;
    gap: 1em;
    padding: 0.2em 0.5em;
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
  }
  li:hover,
  li.on {
    background: var(--selection);
  }
  .tag {
    color: var(--muted);
    font-size: 0.8em;
  }
</style>
