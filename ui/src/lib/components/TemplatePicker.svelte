<script lang="ts">
  // The list behind `/template`: a filter box and the templates and saved blocks of the graph. Arrow keys move,
  // Enter inserts, Escape closes. It takes the focus for the filter box; the parent decides where the focus goes
  // after pick/cancel (like EmojiPicker).
  import { onMount } from 'svelte'
  import { templateChoices, type TemplateChoice } from '../commands/slash'

  let { onpick, oncancel }: { onpick: (template: string) => void; oncancel: () => void } = $props()

  let q = $state('')
  // the position in the filtered list; `$state` so the highlight follows it
  let active = $state(0)
  let input: HTMLInputElement
  const all = templateChoices()
  // `$derived` recomputes when `q` changes. It only reads state; writing `active` happens in `oninput` below
  const shown = $derived(all.filter((c) => c.name.toLowerCase().includes(q.trim().toLowerCase())))
  const kinds = [
    { kind: 'template', title: 'Templates' },
    { kind: 'snippet', title: 'Saved blocks' },
  ] as const
  const sections = $derived(
    kinds.map((k) => ({ ...k, items: shown.filter((c) => c.kind === k.kind) })).filter((s) => s.items.length),
  )

  onMount(() => input.focus())

  function key(e: KeyboardEvent) {
    e.stopPropagation() // keep Enter/Escape/arrows away from the editor and the global handler
    if (e.key === 'Escape') {
      e.preventDefault()
      oncancel()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      if (shown.length) active = (active + (e.key === 'ArrowDown' ? 1 : -1) + shown.length) % shown.length
    } else if (e.key === 'Enter' && shown[active]) {
      e.preventDefault()
      onpick(shown[active].template)
    }
  }
  const isActive = (c: TemplateChoice) => shown[active]?.template === c.template
</script>

<!-- a click inside must not reach the block's own click handler (it would re-open the editor) -->
<div
  class="template-picker"
  role="dialog"
  aria-label="Pick a template"
  tabindex="-1"
  data-testid="template-picker"
  onkeydown={key}
  onclick={(e) => e.stopPropagation()}
>
  <input
    bind:this={input}
    bind:value={q}
    oninput={() => (active = 0)}
    placeholder="Filter templates…"
    aria-label="Filter templates"
  />
  <div class="list" role="listbox" aria-label="Templates">
    {#each sections as s (s.kind)}
      <h4>{s.title}</h4>
      {#each s.items as c (c.template)}
        <button
          type="button"
          role="option"
          aria-selected={isActive(c)}
          class:active={isActive(c)}
          onclick={() => onpick(c.template)}>{c.name}</button
        >
      {/each}
    {:else}
      <p class="none">
        {all.length
          ? 'No template matches.'
          : 'No templates yet. Make one in Settings → Templates, or select blocks and use “Save as template…”.'}
      </p>
    {/each}
  </div>
</div>

<style>
  .template-picker {
    display: flex;
    flex-direction: column;
    gap: 0.4em;
    padding: 0.6em;
    width: 21em;
    max-width: 90vw;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow:
      0 8px 28px rgb(0 0 0 / 0.18),
      0 1px 3px rgb(0 0 0 / 0.1);
    font-size: 0.9em;
  }
  .list {
    max-height: 17em;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  h4 {
    margin: 0.5em 0 0.2em;
    font-size: 0.75em;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  button {
    border: none;
    background: none;
    text-align: left;
    padding: 0.35em 0.5em;
    border-radius: 6px;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  button:hover,
  button.active {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .none {
    margin: 0.4em 0;
    color: var(--muted);
  }
</style>
