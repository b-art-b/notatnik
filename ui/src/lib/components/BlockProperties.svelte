<script lang="ts">
  // All blocks that have a block property (`client`), or one value of it (`client: Acme`). "Include children"
  // adds the blocks below them with the `inherit:` clause (PR-5).
  import QueryBlock from './QueryBlock.svelte'
  import { blockPropertyHash } from '../route'

  let { key, value }: { key: string; value: string | null } = $props()
  let children = $state(false)

  const text = $derived(
    [
      `prop.${key}: ${value ?? 'any'}`,
      children ? `inherit: ${key}` : '',
      value === null ? `group: prop.${key}` : '',
      'sort: page asc',
    ]
      .filter(Boolean)
      .join('\n'),
  )
</script>

<h1>
  <span class="k">{key}</span>{#if value !== null}<span class="sep">:</span>
    <span class="v">{value}</span>{/if}
</h1>
<p class="muted">
  {value === null ? `Blocks by the value of “${key}”.` : `Blocks with ${key}: ${value}.`}
  {#if value !== null}<a href={blockPropertyHash(key)}>All values of {key}</a>{/if}
</p>
<label class="opt">
  <input type="checkbox" bind:checked={children} data-testid="include-children" />
  Include blocks below them
</label>
<QueryBlock {text} forcedView="list" />

<style>
  .k,
  .v {
    font-weight: 700;
  }
  .sep,
  .muted {
    color: var(--muted);
  }
  .opt {
    display: block;
    margin: 0.5em 0 1em;
    font-size: 0.9em;
  }
</style>
