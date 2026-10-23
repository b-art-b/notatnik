<script lang="ts">
  // The page's emoji, if it has one (decorative: the page name is next to it).
  import { anyIcons, iconOf } from '../stores/icons.svelte'

  // `journal`: with no icon of its own, a journal day shows the calendar glyph (a CSS mask, drawn in text colour)
  // `align`: in a list: a page with no icon gets an empty box of the same width while other pages have one, so the
  // names start at the same place. Not for an icon inside a sentence, where the gap would be odd.
  let {
    name,
    journal = false,
    align = false,
  }: { name: string; journal?: boolean; align?: boolean } = $props()
  const icon = $derived(iconOf(name))
</script>

{#if icon}<span class="picon" aria-hidden="true" data-testid="page-icon">{icon}</span>{:else if journal}<span
    class="jicon"
    aria-hidden="true"
  ></span>{:else if align && anyIcons()}<span class="picon blank" aria-hidden="true"></span>{/if}
