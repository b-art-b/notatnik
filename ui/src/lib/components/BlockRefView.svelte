<script lang="ts">
  // `((id))` rendered inline: the referenced block's first paragraph, with a cycle guard.
  import { backend, type BlockLookup } from '../api'
  import { pageHash } from '../route'
  import Inlines from './Inlines.svelte'
  import { untrack } from 'svelte'
  import { getDoc } from '../stores/pages.svelte'
  import { bus } from '../stores/events.svelte'
  import { highlightDoc } from '../annotate/highlightDoc'
  import HighlightChip from './HighlightChip.svelte'

  let { uid, chain }: { uid: string; chain: string[] } = $props()

  let result = $state<BlockLookup | null | undefined>(undefined)
  const cycle = $derived(chain.includes(uid))

  // the block is read again when its page changes (a highlight that was removed, a quote that was edited)
  let again = $state(0)
  $effect(() => {
    const ev = bus.last
    if (ev && untrack(() => result)?.page && ev.pages.includes(untrack(() => result)!.page))
      untrack(() => again++)
  })

  $effect(() => {
    void again
    if (cycle) return
    let cancelled = false
    backend.call('get_block', { uid }).then((r) => {
      if (!cancelled) result = r
    })
    return () => {
      cancelled = true
    }
  })

  // a highlight of a PDF or an EPUB (PA-6, EA-7): the block has `hl-page` or `hl-cfi` and its page says which file it
  // annotates. A small chip names the file and the place, and a click opens the document there.
  // (the page's document is made in a derived of its own: a state read in the same derived that made it is not tracked)
  const sidecar = $derived(result ? getDoc(result.page) : null)
  const doc = $derived.by(() => {
    const b = result?.block
    const file = sidecar?.view?.frontmatter?.props.find((p) => p.key === 'file')?.raw
    return b && result ? highlightDoc(file, b.props, result.page) : null
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
  <a
    class="block-ref"
    href={pageHash(result.page)}
    data-uid={uid}
    title={`^${uid} on ${result.page}`}
    onclick={(e) => e.stopPropagation()}><Inlines items={inlines} chain={[...chain, uid]} /></a
  >{#if doc}<HighlightChip {doc} {uid} />{/if}
{/if}

<style>
  .block-ref {
    border-bottom: 1px dotted var(--muted);
    color: inherit;
    text-decoration: none;
  }
</style>
