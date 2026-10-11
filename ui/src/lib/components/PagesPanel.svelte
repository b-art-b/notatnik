<script lang="ts">
  // The "Pages" part of the left panel: the namespace tree, or the pages grouped by a page property
  // (`type: person` → a "person" group, a "client" group, ...). The choice is remembered per device.
  import { backend, type PageKeyView } from '../api'
  import { bus } from '../stores/events.svelte'
  import NamespaceTree from './NamespaceTree.svelte'
  import GroupedPages from './GroupedPages.svelte'

  const STORE = 'notatnik-pages-group-by'
  let groupBy = $state('') // '' = the namespace tree
  let keys = $state<PageKeyView[]>([])

  try {
    groupBy = localStorage.getItem(STORE) ?? ''
  } catch {
    /* default */
  }
  $effect(() => {
    try {
      localStorage.setItem(STORE, groupBy)
    } catch {
      /* not remembered */
    }
  })

  $effect(() => {
    void bus.version
    let cancelled = false
    backend.call('page_property_keys', {}).then((r) => {
      if (!cancelled) keys = r
    })
    return () => {
      cancelled = true
    }
  })

  // keep the chosen key selectable even if no page has it any more
  const options = $derived(
    groupBy && !keys.some((k) => k.key === groupBy) ? [...keys, { key: groupBy, pages: 0 }] : keys,
  )
</script>

<div class="head">
  <h3>Pages</h3>
  <select aria-label="Group pages by" bind:value={groupBy} title="Group pages by">
    <option value="">by namespace</option>
    {#each options as k (k.key)}<option value={k.key}>by {k.key} ({k.pages})</option>{/each}
  </select>
</div>
{#if groupBy}
  <GroupedPages key={groupBy} />
{:else}
  <NamespaceTree />
{/if}

<style>
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4em;
  }
  .head h3 {
    margin: 0.6em 0;
  }
  select {
    max-width: 55%;
    font-size: 0.8em;
  }
</style>
