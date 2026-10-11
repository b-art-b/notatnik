<script lang="ts">
  // A small set of exclusive choices as a segmented control. Real radio inputs underneath, so the keyboard
  // (arrow keys), screen readers and form semantics all work; the label is the visible chip.
  interface Option {
    value: string
    label: string
  }
  let {
    name,
    legend,
    options,
    value,
    onchange,
  }: { name: string; legend: string; options: Option[]; value: string; onchange: (v: string) => void } =
    $props()
</script>

<div class="seg" role="radiogroup" aria-label={legend}>
  {#each options as o (o.value)}
    <label class:on={value === o.value}>
      <input
        type="radio"
        {name}
        value={o.value}
        checked={value === o.value}
        onchange={() => onchange(o.value)}
      />
      <span>{o.label}</span>
    </label>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  label {
    position: relative;
    cursor: pointer;
  }
  span {
    display: block;
    padding: 0.35em 1em;
  }
  label + label {
    border-left: 1px solid var(--border);
  }
  label.on {
    background: var(--selection);
    font-weight: 600;
  }
  input {
    position: absolute;
    opacity: 0;
    inset: 0;
    cursor: pointer;
  }
  label:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
</style>
