<script lang="ts">
  // The list of highlights beside a PDF or an EPUB (PA-4, EA-5): reading order, colour and text filters, the quote (or
  // the picture of an area), the comments as editable blocks, and a button to add one. The viewer says how a place is
  // called (`p. 12`, `3 · The Storm`) and what a click on a quote does. The test ids keep the `pdf-` prefix of the
  // first viewer.
  import { backend } from '../api'
  import { COLORS, type Color, type Highlight } from '../pdf/highlights'
  import { COLOR_HEX } from '../annotate/colors'
  import { PANEL_VIEW, type Sidecar } from '../annotate/sidecar.svelte'
  import BlockList from './BlockList.svelte'

  let {
    sc,
    place,
    notFound,
    hint,
    onReveal,
    onComment,
    onNotes,
    narrow = false,
  }: {
    sc: Sidecar
    /** How a highlight's place is called in the list. */
    place: (h: Highlight) => string
    /** The line under a highlight that could not be found in the document. */
    notFound: (h: Highlight) => string
    /** What the empty list says. */
    hint: string
    onReveal: (h: Highlight) => void
    onComment: (h: Highlight) => void
    onNotes: () => void
    /** The viewer's box is narrow: the panel is a drawer over the document. */
    narrow?: boolean
  } = $props()

  let colorFilter = $state<Color | ''>('')
  let textFilter = $state('')
  const shown = $derived(
    sc.highlights.filter(
      (h) =>
        (!colorFilter || h.color === colorFilter) &&
        (!textFilter.trim() ||
          `${h.quote} ${h.block.children.map((c) => c.text).join(' ')}`
            .toLowerCase()
            .includes(textFilter.trim().toLowerCase())),
    ),
  )

  const imageOf = (h: Highlight): string => {
    const m = /^!\[[^\]]*\]\(([^)]*)\)/.exec(h.block.text)
    return m ? backend.assetUrl(m[1]) : ''
  }
</script>

<aside class="panel" class:narrow aria-label="Highlights" data-testid="pdf-panel">
  <div class="filters">
    <span class="colors" role="group" aria-label="Filter by colour">
      <button
        type="button"
        class="dot all"
        class:on={colorFilter === ''}
        onclick={() => (colorFilter = '')}
        aria-label="All colours"
        data-testid="pdf-filter-all">all</button
      >
      {#each COLORS as c (c)}
        <button
          type="button"
          class="dot"
          class:on={colorFilter === c}
          style:background={COLOR_HEX[c]}
          onclick={() => (colorFilter = colorFilter === c ? '' : c)}
          aria-label={`Only ${c}`}
          data-testid="pdf-filter-{c}"
        ></button>
      {/each}
    </span>
    <input
      bind:value={textFilter}
      placeholder="Filter"
      aria-label="Filter the highlights"
      data-testid="pdf-filter-text"
    />
  </div>
  <p class="count" data-testid="pdf-count">
    {sc.highlights.length} highlight{sc.highlights.length === 1 ? '' : 's'}
  </p>
  {#if sc.side?.page}
    <button type="button" class="link" onclick={onNotes} data-testid="pdf-notes-link"
      >Open the notes page</button
    >
  {/if}
  <ul class="list">
    {#each shown as h (h.blockId)}
      <li
        class="item"
        class:sel={sc.selected !== '' && sc.selected === h.uid}
        data-testid="pdf-item"
        data-color={h.color}
        data-uid={h.uid ?? ''}
      >
        <button
          type="button"
          class="quote"
          style:--c={COLOR_HEX[h.color]}
          onclick={() => onReveal(h)}
          data-testid="pdf-item-quote"
        >
          <span class="pg">{place(h)}</span>
          {#if h.kind === 'area'}
            <img src={imageOf(h)} alt="Highlighted area, {place(h)}" />
          {:else}
            <span class="q">{h.quote}</span>
          {/if}
        </button>
        {#if sc.missing[h.uid ?? String(h.blockId)]}
          <p class="warn" data-testid="pdf-item-missing">{notFound(h)}</p>
        {/if}
        {#if sc.side && h.block.children.length}
          <div class="comments">
            <BlockList blocks={h.block.children} docName={sc.side.page} viewKey={PANEL_VIEW} />
          </div>
        {/if}
        <button type="button" class="add" onclick={() => onComment(h)} data-testid="pdf-add-comment"
          >＋ Comment</button
        >
      </li>
    {:else}
      <li class="empty">{sc.highlights.length ? 'No highlight matches.' : hint}</li>
    {/each}
  </ul>
</aside>

<style>
  .panel {
    width: 22em;
    max-width: 45%;
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 0.6em;
    gap: 0.4em;
    overflow: hidden;
    background: var(--panel);
  }
  button,
  .filters input {
    font: inherit;
    font-size: 0.9em;
    color: inherit;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: transparent;
    padding: 0.25em 0.7em;
    cursor: pointer;
  }
  .count {
    color: var(--muted);
    font-size: 0.85em;
    margin: 0;
  }
  .filters {
    display: flex;
    gap: 0.5em;
    align-items: center;
    flex-wrap: wrap;
  }
  .filters input {
    flex: 1;
    min-width: 6em;
    cursor: text;
  }
  .colors {
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .dot {
    width: 1.5em;
    height: 1.5em;
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
  }
  .dot.on {
    border-color: var(--text, #111);
    box-shadow: 0 0 0 2px var(--panel);
  }
  .dot.all {
    width: auto;
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.75em;
    border-color: var(--border);
  }
  .link {
    align-self: flex-start;
    border: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.6em;
  }
  .item {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4em 0.5em;
  }
  .item.sel {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  /* the paragraphs of a quote are lines of it */
  .quote .q {
    white-space: pre-line;
  }
  .quote {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    border-left: 4px solid var(--c);
    border-radius: 0;
    padding: 0.1em 0 0.1em 0.5em;
  }
  .pg {
    display: block;
    color: var(--muted);
    font-size: 0.75em;
  }
  .q {
    font-size: 0.9em;
  }
  .quote img {
    max-width: 100%;
    display: block;
  }
  .warn {
    margin: 0.2em 0;
    color: #b45309;
    font-size: 0.8em;
  }
  .comments {
    margin: 0.3em 0 0 0.3em;
  }
  .add {
    margin-top: 0.3em;
    border-color: transparent;
    color: var(--muted);
    font-size: 0.8em;
  }
  .empty {
    color: var(--muted);
    font-size: 0.9em;
  }
  /* a finger needs bigger targets, and the panel covers the page on a narrow window */
  @media (pointer: coarse) {
    button {
      min-height: 44px;
      min-width: 44px;
    }
    .dot {
      width: 44px;
      height: 44px;
    }
  }
  /* a viewer box narrower than 900 px (a docked viewer, a small window): the panel is a drawer over the document */
  .panel.narrow {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: min(22em, 90%);
    max-width: none;
    box-shadow: -4px 0 14px rgb(0 0 0 / 0.3);
    z-index: 20;
  }
</style>
