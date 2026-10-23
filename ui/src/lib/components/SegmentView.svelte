<script lang="ts">
  // Everywhere a name occurs as a path segment: `Renewal` in P/Capgemini/Renewal/Siemens Energy, P/EY/Renewal/APotheke
  // and X/Renewal/Snowflake. Think of every segment as an automatic, indirect tag. The places are grouped by where the
  // segment occurs; below them come the open tasks of all those pages and the explicit references to a page or tag
  // of the same name (`#renewal`, `[[Renewal]]`).
  import { backend, type SegmentView as Data } from '../api'
  import { bus } from '../stores/events.svelte'
  import { pageHash } from '../route'
  import PageIcon from './PageIcon.svelte'
  import PageTasks from './PageTasks.svelte'
  import Backlinks from './Backlinks.svelte'

  let { segment }: { segment: string } = $props()
  let data = $state<Data | null>(null)

  $effect(() => {
    void bus.version
    const s = segment
    let cancelled = false
    backend.call('segment_view', { segment: s }).then((r) => {
      if (!cancelled) data = r
    })
    return () => {
      cancelled = true
    }
  })

  const places = $derived(data?.groups.length ?? 0)
  const pageCount = $derived(data?.groups.reduce((n, g) => n + g.pages.length, 0) ?? 0)

  /** `P/EY/Renewal/APotheke` below `P/EY/Renewal` → `APotheke` */
  const rest = (name: string, occurrence: string) => name.slice(occurrence.length + 1)
</script>

<h1>{segment} <span class="count" data-testid="segment-places">{places}</span></h1>
<p class="muted">
  Pages with “{segment}” in their path: {places === 1 ? 'one place' : `${places} places`}, {pageCount}
  {pageCount === 1 ? 'page' : 'pages'} below.
</p>

{#if data && data.groups.length === 0}
  <p class="muted">No page has “{segment}” in its path.</p>
{/if}

{#each data?.groups ?? [] as g (g.occurrence)}
  <section class="group" data-testid="segment-group" data-occurrence={g.occurrence}>
    <h2>
      {#each g.occurrence.split('/') as part, i (i)}{#if i > 0}<span class="sep">/</span>{/if}<a
          class="page-ref"
          href={pageHash(
            g.occurrence
              .split('/')
              .slice(0, i + 1)
              .join('/'),
          )}
          data-page={g.occurrence
            .split('/')
            .slice(0, i + 1)
            .join('/')}>{part}</a
        >{/each}
      {#if g.occurrence_is_virtual}<span class="tag" title="No page of its own, only the pages below it"
          >no page</span
        >{/if}
    </h2>
    {#if g.pages.length}
      <ul>
        {#each g.pages as p (p.name)}
          <li class:virtual={p.is_virtual}>
            <a href={pageHash(p.name)} data-page={p.name}
              ><PageIcon name={p.name} align />{rest(p.name, g.occurrence)}</a
            >
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted none">Nothing below it.</p>
    {/if}
  </section>
{/each}

<PageTasks {segment} />
<Backlinks page={segment} />

<style>
  .count {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.5em;
    vertical-align: middle;
  }
  h2 {
    font-size: 1em;
    margin: 1.2em 0 0.2em;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.15em;
  }
  .sep {
    color: var(--muted);
    margin: 0 0.45em;
    font-weight: 400;
  }
  .tag {
    font-size: 0.7em;
    font-weight: 400;
    color: var(--muted);
    background: var(--chip);
    border-radius: 6px;
    padding: 0 0.4em;
    margin-left: 0.5em;
  }
  ul {
    margin: 0.3em 0 0.3em 0.4em;
    padding-left: 1.2em;
    list-style: circle;
  }
  li {
    padding: 0.1em 0;
  }
  li.virtual a {
    font-style: italic;
  }
  .none {
    margin: 0.2em 0 0 0.4em;
    font-size: 0.85em;
  }
</style>
