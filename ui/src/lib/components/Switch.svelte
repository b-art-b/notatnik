<script lang="ts">
  // The one switch of Settings: a checkbox drawn as a switch, with `role="switch"` for screen readers. It is for a
  // setting that applies the moment it is flipped; an option that is saved with a button, or sits in a form or a list,
  // stays a plain checkbox. It takes the attributes of an input (`id`, `checked`, `disabled`, `onchange`,
  // `aria-label`, `data-testid`) and passes them on. The style lives here, not in Settings.svelte: Svelte scopes a
  // style to the component that has it, so a switch in a pane of its own would not get it.
  import type { HTMLInputAttributes } from 'svelte/elements'

  // `type`, `role` and `class` are fixed by the component
  let props: Omit<HTMLInputAttributes, 'type' | 'role' | 'class'> = $props()
</script>

<input {...props} type="checkbox" role="switch" class="switch" />

<style>
  .switch {
    appearance: none;
    width: 2.6em;
    height: 1.45em;
    border-radius: 1em;
    background: var(--border);
    position: relative;
    cursor: pointer;
    transition: background 0.15s;
    margin: 0;
    flex: none;
  }
  .switch::after {
    content: '';
    position: absolute;
    top: 0.15em;
    left: 0.15em;
    width: 1.15em;
    height: 1.15em;
    border-radius: 50%;
    background: var(--panel);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.3);
    transition: transform 0.15s;
  }
  .switch:checked {
    background: var(--accent);
  }
  .switch:checked::after {
    transform: translateX(1.15em);
  }
  .switch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .switch:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
