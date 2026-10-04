<script lang="ts">
  // `((id))` rendered inline: the referenced block's first paragraph, with a cycle guard.
  import { backend, type BlockLookup } from '../api'
  import { pageHash } from '../route'
  import Inlines from './Inlines.svelte'

  let { uid, chain }: { uid: string; chain: string[] } = $props()

  let result = $state<BlockLookup | null | undefined>(undefined)
  const cycle = $derived(chain.includes(uid))

  $effect(() => {
    if (cycle) return
    let cancelled = false
    backend.call('get_block', { uid }).then((r) => {
      if (!cancelled) result = r
    })
    return () => {
      cancelled = true
    }
  })

  const inlines = $derived.by(() => {
    const first = result?.block.content[0]
    return first && (first.type === 'paragraph' || first.type === 'heading') ? first.children : []
  })
</script>

{#if cycle}
  <span class="chip error" title="This block refers to itself">⟲ cycle</span>
{:else if result === undefined}
  <span class="chip">…</span>
{:else if result === null}
  <span class="chip error" title="No block with this id">broken ref</span>
{:else}
  <a class="block-ref" href={pageHash(result.page)} data-uid={uid} onclick={(e) => e.stopPropagation()}
    ><Inlines items={inlines} chain={[...chain, uid]} /></a
  >
{/if}

<style>
  .block-ref {
    border-bottom: 1px dotted var(--muted);
    color: inherit;
    text-decoration: none;
  }
</style>
