<script lang="ts">
  // The list of settings sections: grouped by what they are about, each with an icon, and a search box that narrows
  // the list to the sections that hold a setting you type the name of. It is a vertical tablist (arrow keys move
  // between the sections, Tab leaves the list); the section itself is shown by the parent.
  import { untrack } from 'svelte'
  import { SETTINGS_GROUPS, type SectionId, sectionsMatching } from './sections'

  let { tab = $bindable() }: { tab: SectionId } = $props()

  let query = $state('')
  const shown = $derived(sectionsMatching(query))
  const groups = $derived(
    SETTINGS_GROUPS.map((g) => ({ ...g, items: g.items.filter((i) => shown.has(i.id)) })).filter(
      (g) => g.items.length,
    ),
  )
  const flat = $derived(groups.flatMap((g) => g.items))

  // Typing a search shows the first section that matches, unless the open one matches too. `tab` is read without
  // tracking: the effect must run when the search changes, not each time a section is picked.
  $effect(() => {
    const first = flat[0]
    if (query && first && !flat.some((t) => t.id === untrack(() => tab))) tab = first.id
  })

  /** Arrow keys move between the sections (roving tabindex, as for any tablist). */
  function key(e: KeyboardEvent) {
    const i = flat.findIndex((t) => t.id === tab)
    let n = i
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') n = (i + 1) % flat.length
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') n = (i - 1 + flat.length) % flat.length
    else if (e.key === 'Home') n = 0
    else if (e.key === 'End') n = flat.length - 1
    else return
    e.preventDefault()
    tab = flat[n].id
    document.getElementById(`tab-${tab}`)?.focus()
  }
  /** Enter in the search box opens the first section that matches. */
  function searchKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && flat.length) {
      e.preventDefault()
      tab = flat[0].id
    } else if (e.key === 'Escape' && query) {
      e.stopPropagation()
      query = ''
    }
  }
</script>

<div class="nav">
  <input
    class="search"
    type="search"
    placeholder="Search settings"
    aria-label="Search settings"
    bind:value={query}
    onkeydown={searchKey}
  />
  <div class="tabs" role="tablist" aria-label="Settings categories" aria-orientation="vertical" tabindex="-1">
    {#each groups as g (g.label)}
      <div class="group" role="presentation">{g.label}</div>
      {#each g.items as t (t.id)}
        <button
          role="tab"
          id={`tab-${t.id}`}
          aria-selected={tab === t.id}
          aria-controls={`pane-${t.id}`}
          tabindex={tab === t.id || (!flat.some((x) => x.id === tab) && t.id === flat[0].id) ? 0 : -1}
          class:on={tab === t.id}
          onclick={() => (tab = t.id)}
          onkeydown={key}
        >
          <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d={t.icon} /></svg>{t.label}
        </button>
      {/each}
    {:else}
      <p class="none" role="presentation">No setting matches “{query}”.</p>
    {/each}
  </div>
</div>

<style>
  .nav {
    min-width: 0;
    position: sticky;
    top: 1em;
    display: flex;
    flex-direction: column;
    gap: 0.7em;
  }
  .search {
    font: inherit;
    font-size: 0.9em;
    padding: 0.4em 0.7em;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    color: inherit;
    width: 100%;
    box-sizing: border-box;
  }
  .search:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: 0.1em;
  }
  .group {
    margin: 0.9em 0 0.25em 0.8em;
    font-size: 0.7em;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .group:first-child {
    margin-top: 0;
  }
  button {
    display: flex;
    align-items: center;
    gap: 0.65em;
    text-align: left;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    padding: 0.45em 0.8em;
    border-radius: 8px;
    cursor: pointer;
  }
  button:hover {
    background: var(--chip);
  }
  button.on {
    background: var(--selection);
    font-weight: 600;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .icon {
    flex: none;
    width: 1.1em;
    height: 1.1em;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.7;
  }
  button.on .icon {
    opacity: 1;
  }
  .none {
    margin: 0.5em 0.8em;
    color: var(--muted);
    font-size: 0.9em;
  }
  @media (max-width: 760px) {
    .nav {
      position: static;
    }
    /* one scrolling row of sections instead of a column */
    .tabs {
      flex-direction: row;
      overflow-x: auto;
      gap: 0.3em;
      padding-bottom: 0.3em;
    }
    .group {
      display: none;
    }
    button {
      flex: none;
      white-space: nowrap;
      border: 1px solid var(--border);
    }
  }
</style>
