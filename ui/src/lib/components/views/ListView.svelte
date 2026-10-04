<script lang="ts">
  import type { ResultRow } from '../../api'
  import { pageHash } from '../../route'
  import Block from '../Block.svelte'

  let { rows, groupBy, id }: { rows: ResultRow[]; groupBy: string | null; id: string } = $props()
  let collapsed = $state<Record<string, boolean>>({})

  // default grouping is by page; an explicit `group:` uses the labels from the backend
  const groups = $derived.by(() => {
    const m = new Map<string, ResultRow[]>()
    for (const r of rows) {
      const key = groupBy ? (r.group ?? '(none)') : r.page
      m.set(key, [...(m.get(key) ?? []), r])
    }
    return [...m.entries()]
  })
</script>

<div class="list" data-testid="query-list">
  {#each groups as [label, items] (label)}
    <section class="group" data-group={label}>
      <div class="head">
        <button
          class="tog"
          aria-expanded={!collapsed[label]}
          aria-label={`Toggle ${label}`}
          onclick={() => (collapsed[label] = !collapsed[label])}>{collapsed[label] ? '▸' : '▾'}</button
        >
        {#if !groupBy}<a href={pageHash(label)}>{label}</a>{:else}<span>{label}</span>{/if}
        <span class="n">{items.length}</span>
      </div>
      {#if !collapsed[label]}
        {#each items as r (r.id)}
          <div class="item" data-row={r.id}>
            {#if groupBy}<div class="crumbs">
                {r.page}{r.breadcrumbs.length ? ' › ' + r.breadcrumbs.join(' › ') : ''}
              </div>{/if}
            {#if r.block}
              <Block block={r.block} docName={r.page} viewKey={`${id}-${r.id}`} />
            {:else}
              <a href={pageHash(r.page)}>{r.page}</a>
            {/if}
          </div>
        {/each}
      {/if}
    </section>
  {/each}
</div>

<style>
  .head {
    color: var(--muted);
    font-weight: 600;
    padding: 0.2em 0;
  }
  .tog {
    border: none;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .n {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.8em;
    margin-left: 0.3em;
  }
  .crumbs {
    font-size: 0.72em;
    color: var(--muted);
    margin-left: 34px;
  }
</style>
