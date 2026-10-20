<script lang="ts" generics="T extends string">
  // A row of links inside one settings section, for a section that is too long for one scroll (Backup). It is plain
  // navigation, not a second tablist: the buttons say which part is open with `aria-current`, so a page still has
  // exactly one tablist and one tabpanel (the section).
  let {
    items,
    value = $bindable(),
    label,
  }: { items: { id: T; label: string }[]; value: T; label: string } = $props()
</script>

<nav class="sub" aria-label={label}>
  {#each items as i (i.id)}
    <button
      type="button"
      class:on={value === i.id}
      aria-current={value === i.id ? 'page' : undefined}
      onclick={() => (value = i.id)}>{i.label}</button
    >
  {/each}
</nav>

<style>
  .sub {
    display: flex;
    flex-wrap: wrap;
    gap: 0.2em 1.2em;
    margin: 0.4em 0 0.8em;
    border-bottom: 1px solid var(--border);
  }
  .sub button {
    font: inherit;
    padding: 0.45em 0.1em;
    border: none;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .sub button:hover {
    color: var(--text);
  }
  .sub button.on {
    color: var(--text);
    font-weight: 600;
    border-bottom-color: var(--accent);
  }
  .sub button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 4px;
  }
</style>
