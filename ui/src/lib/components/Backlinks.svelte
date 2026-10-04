<script lang="ts">
  import { bus } from '../stores/events.svelte'
  import { backend, type BacklinkView } from '../api'
  import { pageHash } from '../route'
  import Nodes from './Nodes.svelte'

  let { page }: { page: string } = $props()
  let links = $state<BacklinkView[]>([])
  let open = $state(true)

  $effect(() => {
    void bus.version
    let cancelled = false
    backend.call('backlinks', { page }).then((r) => {
      if (!cancelled) links = r
    })
    return () => {
      cancelled = true
    }
  })

  const groups = $derived.by(() => {
    const m = new Map<string, BacklinkView[]>()
    for (const l of links) m.set(l.source_page, [...(m.get(l.source_page) ?? []), l])
    return [...m.entries()]
  })
</script>

<section class="backlinks" aria-label="Linked references">
  <button class="head" onclick={() => (open = !open)} aria-expanded={open}
    >{open ? '▾' : '▸'} Linked references
    <span class="count" data-testid="backlink-count">{links.length}</span></button
  >
  {#if open}
    {#each groups as [source, items] (source)}
      <div class="group" data-source={source}>
        <a class="src" href={pageHash(source)}>{source}</a>
        {#each items as l, i (i)}
          <div class="item">
            {#if l.breadcrumbs.length}<div class="crumbs">{l.breadcrumbs.join(' › ')}</div>{/if}
            {#if l.block}<div class="text"><Nodes nodes={l.block.content} /></div>{:else}<div
                class="text muted"
              >
                ({l.kind} in page properties)
              </div>{/if}
          </div>
        {/each}
      </div>
    {/each}
  {/if}
</section>

<style>
  .backlinks {
    margin-top: 2em;
    border-top: 1px solid var(--border);
    padding-top: 0.6em;
  }
  .head {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    font-weight: 600;
    padding: 0;
  }
  .count {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.85em;
  }
  .group {
    margin: 0.6em 0;
  }
  .src {
    font-weight: 600;
  }
  .item {
    margin: 0.2em 0 0.2em 0.8em;
    padding-left: 0.6em;
    border-left: 2px solid var(--border);
  }
  .crumbs {
    font-size: 0.75em;
    color: var(--muted);
  }
</style>
