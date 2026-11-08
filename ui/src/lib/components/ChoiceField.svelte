<script lang="ts">
  // One question of a template with a fixed list of answers (`{{askchoice:Label|a|b|c}}`, TC-5). Up to five choices
  // are a radio group (the browser moves the choice with ↑/↓); more are a filter field with the matching choices
  // under it. `value` is the chosen text and always one of the choices, or '' when the filter matches none.
  let {
    label,
    choices,
    value = $bindable(''),
    first = false,
  }: { label: string; choices: string[]; value: string; first?: boolean } = $props()

  const SMALL = 5
  const group = `choice-${Math.random().toString(36).slice(2, 8)}` // radio buttons of one question share a name

  let filter = $state('')
  let active = $state(0)
  let input: HTMLInputElement | undefined = $state()
  let radios: HTMLInputElement[] = $state([])
  // focus the question that comes first: its filter field, or the chosen radio button
  $effect(() => {
    if (!first) return
    if (choices.length > SMALL) input?.focus()
    else radios[Math.max(0, choices.indexOf(value))]?.focus()
  })

  const matches = $derived(choices.filter((c) => c.toLowerCase().includes(filter.trim().toLowerCase())))
  function narrow() {
    active = 0
    value = matches[0] ?? ''
  }
  function move(by: number) {
    if (matches.length === 0) return
    active = (active + by + matches.length) % matches.length
    value = matches[active]
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      move(e.key === 'ArrowDown' ? 1 : -1)
    } else if (e.key === 'Enter' && matches.length === 0) {
      e.preventDefault() // nothing to pick: do not insert
    }
  }
</script>

{#if choices.length <= SMALL}
  <fieldset class="small" role="radiogroup" aria-label={label}>
    <legend>{label}</legend>
    {#each choices as c, i (c)}
      <label class="radio">
        <input type="radio" name={group} value={c} bind:group={value} bind:this={radios[i]} />
        {c}
      </label>
    {/each}
  </fieldset>
{:else}
  <div class="big">
    <label>
      {label}
      <input
        bind:this={input}
        bind:value={filter}
        oninput={narrow}
        onkeydown={key}
        role="combobox"
        aria-expanded="true"
        aria-controls="{group}-list"
        aria-label={label}
        placeholder="Type to narrow the list"
      />
    </label>
    <ul id="{group}-list" role="listbox" aria-label="{label}: choices">
      {#each matches as c, i (c)}
        <li role="option" aria-selected={i === active}>
          <button
            type="button"
            class:active={i === active}
            tabindex="-1"
            onclick={() => {
              active = i
              value = c
            }}>{c}</button
          >
        </li>
      {:else}
        <li class="none" role="presentation">Nothing matches</li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  fieldset {
    border: none;
    padding: 0;
    margin: 0 0 0.6em;
  }
  legend {
    padding: 0;
    margin-bottom: 0.2em;
  }
  .radio {
    display: flex;
    align-items: center;
    gap: 0.5em;
    min-height: 44px;
    cursor: pointer;
  }
  .radio input {
    width: auto;
    margin: 0;
  }
  .big {
    margin-bottom: 0.6em;
  }
  input[role='combobox'] {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 0.5em;
    margin-top: 0.2em;
    font-size: 1em;
  }
  ul {
    list-style: none;
    margin: 0.3em 0 0;
    padding: 0;
    max-height: 12em;
    overflow: auto;
    border: 1px solid var(--border, rgba(128, 128, 128, 0.35));
    border-radius: 6px;
  }
  li button {
    display: block;
    width: 100%;
    min-height: 44px;
    text-align: left;
    background: none;
    border: none;
    font: inherit;
    color: inherit;
    padding: 0.4em 0.6em;
    cursor: pointer;
  }
  li button.active {
    background: var(--selection);
  }
  .none {
    padding: 0.5em 0.6em;
    color: var(--muted);
  }
</style>
