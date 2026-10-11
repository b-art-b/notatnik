<script lang="ts">
  import type { BlockView } from '../api'
  import Block from './Block.svelte'

  interface Props {
    blocks: BlockView[]
    docName: string
    viewKey: string
    readonly?: boolean
    chain?: string[]
    basePath?: number[]
  }
  let { blocks, docName, viewKey, readonly = false, chain = [] }: Props = $props()
</script>

{#each blocks as block (block.id)}
  <Block {block} {docName} {viewKey} {readonly} {chain} compact />
{/each}
