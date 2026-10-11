<script lang="ts">
  // A task shown outside its note, with where it lives. Used by the Agenda sections and the page's Tasks section.
  // Layout (UX): the source sits in the task's own first row, right-aligned, like the sender column of a mail
  // list. It can only belong to the task it is next to, which a line above the task never could.
  import type { BlockView } from '../api'
  import Block from './Block.svelte'
  import { jumpOnClick } from '../jump'
  import { pageHash } from '../route'
  import { sourceLabel, sourceTitle } from '../agenda'
  import { shownDate } from '../stores/format.svelte'

  // `$props()` is how a Svelte 5 component receives its inputs; destructuring names them.
  // `relativeTo`: the page whose subpages are listed (the chip then shows the name below it).
  // `chip`: the chip's text when it is not the source note's name (`null`: no chip).
  // `tip`: tooltip override.
  let {
    block,
    page,
    breadcrumbs,
    viewKey,
    relativeTo = '',
    chip = undefined,
    tip = undefined,
    itemTestid = 'agenda-item',
    sourceTestid = 'agenda-source',
  }: {
    block: BlockView
    page: string
    breadcrumbs: string[]
    viewKey: string
    relativeTo?: string
    chip?: string | null
    tip?: string
    itemTestid?: string
    sourceTestid?: string
  } = $props()

  const label = $derived(chip === undefined ? sourceLabel(page, relativeTo, shownDate) : chip)
  const title = $derived(tip ?? sourceTitle(page, breadcrumbs))
</script>

<div class="sourced" data-testid={itemTestid}>
  <div class="task" role="tree" aria-label={`Task in ${page}`}>
    <Block {block} docName={page} {viewKey} />
  </div>
  <!-- opens the note and flashes the task; the full path is in the tooltip -->
  {#if label}
    <a
      class="source"
      href={pageHash(page)}
      {title}
      aria-label={`Open ${title}`}
      data-testid={sourceTestid}
      onclick={(e) => jumpOnClick(e, page, block.path)}>{label}</a
    >
  {/if}
</div>

<style>
  .sourced {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    column-gap: 0.9em;
    align-items: start;
    padding: 0.35em 0;
  }
  /* a hairline between items: each task reads as one unit */
  :global(.sourced + .sourced) {
    border-top: 1px solid var(--border);
  }
  .task {
    grid-column: 1;
    grid-row: 1;
    min-width: 0;
  }
  .source {
    grid-column: 2;
    grid-row: 1;
    max-width: 14em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 0.15em;
    padding: 0.05em 0.55em;
    border-radius: 999px;
    background: var(--chip);
    color: var(--muted);
    font-size: 0.8em;
    line-height: 1.6;
    text-decoration: none;
  }
  .source:hover {
    color: var(--link);
  }
  /* narrow window: the source drops under the task instead of squeezing it */
  @media (max-width: 560px) {
    .sourced {
      grid-template-columns: minmax(0, 1fr);
    }
    .source {
      grid-column: 1;
      grid-row: 2;
      justify-self: start;
      margin: 0 0 0 34px;
    }
  }
</style>
