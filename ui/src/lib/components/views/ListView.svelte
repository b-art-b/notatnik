<script lang="ts">
  import type { ResultRow } from '../../api'
  import { pageHash } from '../../route'
  import Block from '../Block.svelte'
  import { shownDate } from '../../stores/format.svelte'

  const isDate = (n: string) => /^\d{4}-\d{2}-\d{2}$/.test(n)
  /** `a/b/c` → parents `a`, `b` and the leaf `c`. */
  const parts = (n: string) => {
    const p = n.split('/')
    return { parents: p.slice(0, -1), leaf: p[p.length - 1] }
  }

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
        {#if !groupBy}
          <!-- a path reads as muted parents and a strong last part: `Notes /` `Ideas` -->
          <a class="title" href={pageHash(label)} title={label}
            >{#if isDate(label)}{shownDate(label)}{:else}{#each parts(label).parents as p, i (i)}<span
                  class="parent">{p}<span aria-hidden="true"> / </span></span
                >{/each}<span class="leaf">{parts(label).leaf}</span>{/if}</a
          >
        {:else}<span class="title">{label}</span>{/if}
        <span class="n">{items.length}</span>
      </div>
      {#if !collapsed[label]}
        <!-- the rows hang under the title: one rail on the left, the bullets inside it -->
        <div class="rows">
          {#each items as r (r.id)}
            <div class="item" data-row={r.id}>
              {#if groupBy}<div class="crumbs">
                  {r.page}{r.breadcrumbs.length ? ' › ' + r.breadcrumbs.join(' › ') : ''}
                </div>{/if}
              {#if r.block}
                <Block block={r.block} docName={r.page} viewKey={`${id}-${r.id}`} />
              {:else}
                <a href={pageHash(r.page)}>{r.page}</a>
                {#each r.page_blocks as b (b.path.join('.'))}
                  <Block block={b} docName={r.page} viewKey={`${id}-${r.id}`} />
                {/each}
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/each}
</div>

<style>
  .group + .group {
    margin-top: 0.9em;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.25em;
    padding: 0.15em 0 0.25em;
    border-bottom: 1px solid var(--border);
  }
  .tog {
    flex: none;
    width: 1.6em;
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0;
  }
  .title {
    color: var(--text);
    font-weight: 600;
    text-decoration: none;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  a.title:hover .leaf {
    text-decoration: underline;
  }
  .parent {
    color: var(--muted);
    font-weight: 400;
  }
  .n {
    flex: none;
    background: var(--chip);
    color: var(--muted);
    border-radius: 999px;
    padding: 0 0.55em;
    font-size: 0.75em;
    line-height: 1.7;
    margin-left: 0.2em;
  }
  /* the rows are indented to the title's text, and a thin rail shows which page they belong to */
  .rows {
    margin: 0.2em 0 0 0.8em;
    padding-left: 0.7em;
    border-left: 2px solid var(--chip);
  }
  .item {
    padding: 0.1em 0.3em;
    border-radius: 6px;
  }
  .item:hover {
    background: color-mix(in srgb, var(--chip) 55%, transparent);
  }
  .crumbs {
    font-size: 0.72em;
    color: var(--muted);
    margin-left: 22px;
  }
</style>
