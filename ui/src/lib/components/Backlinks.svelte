<script lang="ts">
  import { bus } from '../stores/events.svelte'
  import { backend, type BacklinkView, type UnlinkedView } from '../api'
  import { getDoc } from '../stores/pages.svelte'
  import { errorMessage, toast } from '../stores/app.svelte'
  import { pageHash } from '../route'
  import Nodes from './Nodes.svelte'
  import Block from './Block.svelte'
  import { jumpOnClick } from '../jump'

  let { page }: { page: string } = $props()
  let links = $state<BacklinkView[]>([])
  let open = $state(true)
  // Unlinked references: plain-text mentions. Computed only while the panel is open (it searches the whole graph).
  let unlinkedOpen = $state(false)
  let unlinked = $state<UnlinkedView[]>([])
  let unlinkedLoaded = $state(false)

  $effect(() => {
    void bus.version
    if (!unlinkedOpen) return
    let cancelled = false
    backend.call('unlinked_refs', { page }).then((r) => {
      if (!cancelled) {
        unlinked = r
        unlinkedLoaded = true
      }
    })
    return () => {
      cancelled = true
    }
  })

  /** Turns the first mention in that block into a link (one guarded `update_text`, undoable). */
  async function link(u: UnlinkedView) {
    try {
      await getDoc(u.source_page).apply([
        { op: 'update_text', block: { path: u.block_path, hash: u.block.hash }, text: u.link_text },
      ])
      unlinked = unlinked.filter((x) => x !== u)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

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
          <div class="item" data-testid="backlink-item">
            {#if l.breadcrumbs.length && l.block_path}
              <!-- entry i of the path is the ancestor block at block_path[..=i]; a click opens the page there -->
              <div class="crumbs">
                {#each l.breadcrumbs as crumb, c (c)}
                  {#if c > 0}<span aria-hidden="true"> › </span>{/if}<a
                    href={pageHash(source)}
                    onclick={(e) => jumpOnClick(e, source, l.block_path!.slice(0, c + 1))}>{crumb}</a
                  >
                {/each}
              </div>
            {/if}
            {#if l.block}
              <!-- the whole block with its sub-bullets, editable in place (LN-10) -->
              <div role="tree" aria-label={`Reference in ${source}`}>
                <Block
                  block={l.block}
                  docName={source}
                  viewKey={`ref-${page}-${source}-${l.block_path?.join('.')}`}
                />
              </div>
            {:else}<div class="text muted">({l.kind} in page properties)</div>{/if}
          </div>
        {/each}
      </div>
    {/each}
  {/if}
</section>

<section class="backlinks unlinked" aria-label="Unlinked references">
  <button
    class="head"
    onclick={() => (unlinkedOpen = !unlinkedOpen)}
    aria-expanded={unlinkedOpen}
    data-testid="unlinked-toggle"
    >{unlinkedOpen ? '▾' : '▸'} Unlinked references
    {#if unlinkedLoaded}<span class="count" data-testid="unlinked-count">{unlinked.length}</span>{/if}</button
  >
  {#if unlinkedOpen}
    {#if unlinkedLoaded && unlinked.length === 0}<p class="muted">No plain-text mentions.</p>{/if}
    {#each unlinked as u (u.source_page + u.block_path.join('.'))}
      <div class="item" data-testid="unlinked-item">
        <a class="src" href={pageHash(u.source_page)}>{u.source_page}</a>
        {#if u.breadcrumbs.length}<div class="crumbs">{u.breadcrumbs.join(' › ')}</div>{/if}
        <div class="text"><Nodes nodes={u.block.content} /></div>
        <button class="link-btn" onclick={() => link(u)} title="Turn the mention into [[{page}]]">Link</button
        >
      </div>
    {/each}
  {/if}
</section>

<style>
  .backlinks {
    margin-top: 2.4em;
    padding-top: 0.6em;
  }
  .head {
    border: none;
    background: none;
    color: var(--text);
    cursor: pointer;
    font-weight: 700;
    font-size: 1.05em;
    padding: 0;
  }
  .count {
    background: var(--chip);
    border-radius: 8px;
    padding: 0 0.5em;
    font-size: 0.85em;
  }
  .group {
    margin: 0.8em 0;
    background: var(--card);
    border-radius: 8px;
    padding: 0.7em 1.1em 0.5em;
  }
  .src {
    display: inline-block;
    font-weight: 500;
    text-decoration: none;
    margin-bottom: 0.2em;
  }
  .src:hover {
    text-decoration: underline;
  }
  .item {
    margin: 0.2em 0;
  }
  .unlinked {
    margin-top: 0.8em;
  }
  .link-btn {
    font-size: 0.8em;
    margin-top: 0.2em;
    cursor: pointer;
  }
  .crumbs {
    font-size: 0.75em;
    color: var(--muted);
  }
  .crumbs a {
    color: inherit;
    text-decoration: none;
  }
  .crumbs a:hover {
    color: var(--link);
    text-decoration: underline;
  }
</style>
