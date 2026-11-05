<script lang="ts">
  // The small menu of a highlight that was clicked (PA-3, EA-4), shared by the PDF viewer and the EPUB reader.
  import { COLORS, type Color, type Highlight } from '../pdf/highlights'
  import { COLOR_HEX } from '../annotate/colors'

  let {
    h,
    x,
    y,
    onColor,
    onComment,
    onRef,
    onQuote,
    onDelete,
  }: {
    h: Highlight
    x: number
    y: number
    onColor: (c: Color) => void
    onComment: () => void
    onRef: () => void
    onQuote: () => void
    onDelete: () => void
  } = $props()
</script>

<div
  class="popup"
  style:left="{x}px"
  style:top="{y}px"
  role="dialog"
  aria-label="Highlight"
  data-testid="pdf-popup"
>
  <span class="colors" role="group" aria-label="Colour">
    {#each COLORS as c (c)}
      <button
        type="button"
        class="dot"
        class:on={h.color === c}
        style:background={COLOR_HEX[c]}
        onclick={() => onColor(c)}
        aria-label={c}
        data-testid="pdf-popup-{c}"
      ></button>
    {/each}
  </span>
  <button type="button" onclick={onComment} data-testid="pdf-popup-comment">Add comment</button>
  {#if h.uid}
    <button type="button" onclick={onRef} data-testid="pdf-popup-ref">Copy reference</button>
    {#if h.kind !== 'area'}
      <button type="button" onclick={onQuote} data-testid="pdf-popup-quote">Copy as quote</button>
    {/if}
  {/if}
  <button type="button" class="danger" onclick={onDelete} data-testid="pdf-popup-delete">Delete</button>
</div>

<style>
  .popup {
    position: fixed;
    z-index: 90;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    min-width: 12em;
    padding: 6px 8px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.3);
  }
  button {
    font: inherit;
    font-size: 0.9em;
    color: inherit;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: transparent;
    padding: 0.25em 0.7em;
    cursor: pointer;
  }
  .danger {
    color: #c0392b;
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
</style>
