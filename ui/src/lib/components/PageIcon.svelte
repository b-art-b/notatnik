<script lang="ts">
  // The page's emoji, if it has one (decorative: the page name is next to it).
  import { iconOf } from '../stores/icons.svelte'

  // `journal`: with no icon of its own, a journal day shows the calendar glyph (a CSS mask, drawn in text colour)
  let { name, journal = false }: { name: string; journal?: boolean } = $props()
  const icon = $derived(iconOf(name))
</script>

{#if icon}<span class="picon" aria-hidden="true" data-testid="page-icon">{icon}</span>{:else if journal}<span
    class="jicon"
    aria-hidden="true"
  ></span>{/if}

<style>
  .picon {
    margin-right: 0.25em;
  }
</style>
