<script lang="ts">
  // The bar next to a selection (PA-3, EA-4): the five colours and Copy. It is shared by the PDF viewer and the EPUB
  // reader, which position it (`x`, `y` are the middle of the top edge of the selection, in window pixels).
  // The test ids keep the `pdf-` prefix of the first viewer.
  import { COLORS, type Color } from '../pdf/highlights'
  import { COLOR_HEX } from '../annotate/colors'

  let { x, y, onColor, onCopy }: { x: number; y: number; onColor: (c: Color) => void; onCopy: () => void } =
    $props()
</script>

<div
  class="selbar"
  style:left="{x}px"
  style:top="{y}px"
  role="group"
  aria-label="Highlight the selection"
  data-testid="pdf-selbar"
  onpointerdown={(e) => e.preventDefault()}
>
  {#each COLORS as c (c)}
    <button
      type="button"
      class="dot"
      style:background={COLOR_HEX[c]}
      onclick={() => onColor(c)}
      aria-label={`Highlight ${c}`}
      data-testid="pdf-color-{c}"
    ></button>
  {/each}
  <button type="button" class="copy" onclick={onCopy} data-testid="pdf-copy">Copy</button>
</div>

<style>
  .selbar {
    position: fixed;
    z-index: 90;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.3);
    transform: translate(-50%, calc(-100% - 8px));
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
  .dot {
    width: 1.5em;
    height: 1.5em;
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
  }
  /* a finger needs bigger targets */
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
