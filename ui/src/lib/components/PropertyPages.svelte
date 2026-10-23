<script lang="ts">
  // All pages that have a page property: `type` lists them grouped by value, `type` + `person` only the pages
  // with that value. The other values of the property are one click away.
  import { backend, type PageGroupView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { pageHash, propertyHash } from '../route'

  let { key, value }: { key: string; value: string | null } = $props()
  let groups = $state<PageGroupView[]>([])
  let loaded = $state(false)

  $effect(() => {
    void bus.version
    const k = key
    let cancelled = false
    loaded = false
    backend.call('page_groups', { key: k }).then((r) => {
      if (!cancelled) {
        groups = r
        loaded = true
      }
    })
    return () => {
      cancelled = true
    }
  })

  const same = (a: string | null, b: string | null) => (a ?? '').toLowerCase() === (b ?? '').toLowerCase()
  const valued = $derived(groups.filter((g) => g.value !== null))
  // one value: just that group; no value: every group that has a value, then the pages without one
  const shown = $derived(value === null ? groups : groups.filter((g) => same(g.value, value)))
  const total = $derived(shown.reduce((n, g) => n + g.pages.length, 0))
  import PageIcon from './PageIcon.svelte'
</script>

<h1>
  <span class="k">{key}</span>{#if value !== null}<span class="sep">:</span>
    <span class="v">{value}</span>{/if}
  <span class="count" data-testid="property-count">{total}</span>
</h1>
<p class="muted">
  {value === null ? `Pages by the value of “${key}”.` : `Pages with ${key}: ${value}.`}
  {#if value !== null}<a href={propertyHash(key)}>All values of {key}</a>{/if}
</p>

{#if loaded && shown.length === 0}
  <p class="muted">No page has this property.</p>
{/if}
{#each shown as g (g.value ?? '\u0000none')}
  <section class="group" data-group={g.value ?? ''}>
    {#if value === null}
      <h2>
        {#if g.value !== null}<a href={propertyHash(key, g.value)}>{g.value}</a>{:else}<span class="muted"
            >no {key}</span
          >{/if}
        <span class="count">{g.pages.length}</span>
      </h2>
    {/if}
    <ul>
      {#each g.pages as p (p)}<li>
          <a href={pageHash(p)} data-page={p}><PageIcon name={p} align />{p}</a>
        </li>{/each}
    </ul>
  </section>
{/each}

{#if value !== null && valued.length > 1}
  <p class="others">
    Other values:
    {#each valued.filter((g) => !same(g.value, value)) as g (g.value)}
      <a class="chip" href={propertyHash(key, g.value)}
        >{g.value} <span class="count">{g.pages.length}</span></a
      >
    {/each}
  </p>
{/if}

<style>
  .k,
  .v {
    font-weight: 700;
  }
  .sep {
    color: var(--muted);
  }
  .count {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.55em;
    font-weight: 400;
    vertical-align: middle;
  }
  h2 {
    font-size: 1.05em;
    margin: 1em 0 0.2em;
    border-bottom: 1px solid var(--border);
  }
  h2 .count {
    font-size: 0.75em;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0.3em 0;
  }
  li {
    padding: 0.15em 0;
  }
  .others {
    margin-top: 1.5em;
    color: var(--muted);
  }
  .chip {
    display: inline-block;
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.6em;
    margin-right: 0.3em;
    text-decoration: none;
    color: inherit;
  }
</style>
