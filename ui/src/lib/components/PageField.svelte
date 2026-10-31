<script lang="ts">
  // A text field that suggests the pages of the graph as you type. Picking one fills in its name; a name that is
  // not a page yet stays as typed (the "new page" row), and linking it makes the page when something is written.
  import { backend, type TitleHit } from '../api'

  let { value = $bindable(''), first = false }: { value: string; first?: boolean } = $props()

  let hits = $state<TitleHit[]>([])
  let open = $state(false)
  let active = $state(0)
  let input: HTMLInputElement | undefined = $state()
  $effect(() => {
    if (first) input?.focus()
  })

  const typed = $derived(value.trim())
  // the "new page" row is offered unless the typed name is exactly an existing page
  const isNew = $derived(typed !== '' && !hits.some((h) => h.name.toLowerCase() === typed.toLowerCase()))
  const rows = $derived([...hits.map((h) => h.name), ...(isNew ? [typed] : [])])

  let seq = 0
  async function refresh() {
    const mine = ++seq
    const text = typed
    const found = text
      ? await backend.call('search_titles', { text, limit: 8 })
      : (await backend.call('list_namespace', { parent: null })).slice(0, 8).map((r) => ({
          name: r.name,
          is_virtual: r.is_virtual,
        }))
    if (mine === seq) {
      hits = found
      active = 0
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      open = true
      active = (active + (e.key === 'ArrowDown' ? 1 : -1) + rows.length) % Math.max(rows.length, 1)
    } else if (e.key === 'Enter' && open && rows.length) {
      // the first Enter takes the highlighted page; the form is submitted by the next one
      e.preventDefault()
      e.stopPropagation()
      pick(rows[active])
    } else if (e.key === 'Escape' && open) {
      e.stopPropagation() // closes the list, not the dialog
      open = false
    }
  }
  function pick(name: string) {
    value = name
    open = false
  }
</script>

<div class="field">
  <input
    bind:this={input}
    bind:value
    role="combobox"
    aria-expanded={open}
    aria-controls="page-field-list"
    aria-autocomplete="list"
    autocomplete="off"
    onfocus={() => {
      open = true
      void refresh()
    }}
    oninput={() => {
      open = true
      void refresh()
    }}
    onblur={(e) => {
      // not when a button is pressed: closing the list would move the button away before the click lands
      if (e.relatedTarget instanceof HTMLInputElement) open = false
    }}
    onkeydown={key}
  />
  {#if open && rows.length}
    <ul id="page-field-list" role="listbox" aria-label="Pages" data-testid="page-field-list">
      {#each rows as name, i (name + i)}
        <!-- mousedown, not click: the input would lose focus (and close the list) before a click lands -->
        <li
          role="option"
          aria-selected={i === active}
          class:active={i === active}
          onmousedown={(e) => {
            e.preventDefault()
            pick(name)
          }}
        >
          {#if isNew && i === rows.length - 1}<span class="new">New page</span> {name}{:else}{name}{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  input {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 0.5em;
    margin-top: 0.2em;
    font-size: 1em;
  }
  /* in the flow, not floating: it pushes the buttons below it down instead of covering them */
  ul {
    margin: 0.15em 0 0;
    padding: 0.2em;
    list-style: none;
    max-height: 11em;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  li {
    padding: 0.35em 0.6em;
    border-radius: 6px;
    cursor: pointer;
  }
  li.active {
    background: var(--chip);
  }
  .new {
    font-size: 0.8em;
    font-weight: 650;
    color: var(--muted);
    margin-right: 0.3em;
  }
</style>
