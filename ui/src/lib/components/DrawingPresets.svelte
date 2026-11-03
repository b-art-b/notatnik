<script lang="ts">
  // The pen presets of the drawing editor (DR-15): round buttons that set the pen's colour, width and opacity in one
  // tap. A press that is held (or the right button) stores the pen as it is now in that slot. A press and its release
  // are two events, so `held` remembers that the long press already did its work and the click that follows must not
  // apply the old preset over it.
  import { describePreset, type Preset } from '../drawing/presets'

  let {
    presets,
    active,
    onApply,
    onStore,
  }: {
    presets: readonly Preset[]
    /** the slot whose pen is the pen now, -1 for none */
    active: number
    onApply: (i: number) => void
    onStore: (i: number) => void
  } = $props()

  let timer: ReturnType<typeof setTimeout> | null = null
  let held = false

  function down(i: number) {
    held = false
    timer = setTimeout(() => {
      held = true
      onStore(i)
    }, 600)
  }
  function cancel() {
    if (timer) clearTimeout(timer)
    timer = null
  }
  function click(i: number) {
    cancel()
    if (held) held = false
    else onApply(i)
  }
  /** the dot shows the width, from a pin prick to the size of the button */
  const dot = (w: number) => Math.min(18, Math.max(5, w * 1.1))
</script>

<div class="presets" role="group" aria-label="Pens" data-testid="drawing-presets">
  {#each presets as p, i (i)}
    <button
      type="button"
      class:active={active === i}
      aria-pressed={active === i}
      aria-label="Pen {i + 1}: {describePreset(p)}"
      title="{describePreset(p)}. Hold to store the current pen here."
      data-testid="drawing-preset-{i}"
      onpointerdown={() => down(i)}
      onpointerup={cancel}
      onpointerleave={cancel}
      onpointercancel={cancel}
      onclick={() => click(i)}
      oncontextmenu={(e) => {
        e.preventDefault()
        cancel()
        onStore(i)
      }}
    >
      <span
        class="dot"
        style:width="{dot(p.width)}px"
        style:height="{dot(p.width)}px"
        style:background={p.color}
        style:opacity={p.alpha}
      ></span>
    </button>
  {/each}
</div>

<style>
  .presets {
    display: flex;
    gap: 2px;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2px;
  }
  button {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    touch-action: manipulation;
  }
  button:hover {
    background: var(--chip);
  }
  button.active {
    background: var(--chip);
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .dot {
    display: block;
    border-radius: 50%;
  }
  /* a finger needs 44 px */
  @media (pointer: coarse) {
    button {
      width: 44px;
      height: 44px;
    }
  }
</style>
