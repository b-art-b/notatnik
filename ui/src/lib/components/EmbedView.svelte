<script lang="ts">
  // `!((id))` (a block with its children) and `![[page]]` (a whole page), read-only, with a cycle guard.
  import { backend, type BlockLookup, type PageView } from '../api'
  import { pageHash } from '../route'
  import BlockList from './BlockList.svelte'

  let { kind, target, chain }: { kind: 'block' | 'page'; target: string; chain: string[] } = $props()

  const key = $derived(`${kind}:${target}`)
  const cycle = $derived(chain.includes(key))
  let block = $state<BlockLookup | null | undefined>(undefined)
  let page = $state<PageView | null | undefined>(undefined)

  $effect(() => {
    if (cycle) return
    let cancelled = false
    if (kind === 'block') {
      backend.call('get_block', { uid: target }).then((r) => {
        if (!cancelled) block = r
      })
    } else {
      backend.call('get_page', { name: target }).then((p) => {
        if (!cancelled) page = p.exists ? p : null
      })
    }
    return () => {
      cancelled = true
    }
  })
</script>

{#if cycle}
  <div class="embed"><span class="chip error">⟲ embed cycle</span></div>
{:else if kind === 'block'}
  <div class="embed" data-embed={target}>
    {#if block === undefined}
      <span class="chip">…</span>
    {:else if block === null}
      <span class="chip error">broken embed</span>
    {:else}
      <BlockList
        blocks={[block.block]}
        docName={block.page}
        viewKey={`embed-${target}`}
        readonly
        chain={[...chain, key]}
        basePath={[]}
      />
    {/if}
  </div>
{:else}
  <div class="embed" data-embed-page={target}>
    {#if page === undefined}
      <span class="chip">…</span>
    {:else if page === null}
      <span class="chip error">broken embed</span>
    {:else}
      <a class="embed-title" href={pageHash(page.name)}>{page.title}</a>
      <BlockList
        blocks={page.blocks}
        docName={page.name}
        viewKey={`embed-${target}`}
        readonly
        chain={[...chain, key]}
        basePath={[]}
      />
    {/if}
  </div>
{/if}

<style>
  .embed {
    display: block;
    margin: 0.2em 0;
    padding: 0.3em 0.6em;
    border-left: 3px solid var(--accent);
    background: var(--embed-bg);
    border-radius: 4px;
  }
  .embed-title {
    font-weight: 600;
    font-size: 0.85em;
    color: var(--muted);
    text-decoration: none;
  }
</style>
