<script lang="ts">
  import type { Node } from '../api'
  import Inlines from './Inlines.svelte'
  import Self from './Nodes.svelte'

  let { nodes, chain = [] }: { nodes: Node[]; chain?: string[] } = $props()
</script>

{#each nodes as n, i (i)}
  {#if n.type === 'paragraph'}
    <p><Inlines items={n.children} {chain} /></p>
  {:else if n.type === 'heading'}
    <svelte:element this={`h${Math.min(n.level, 6)}`} class="md-heading"><Inlines items={n.children} {chain} /></svelte:element>
  {:else if n.type === 'code_block'}
    <pre class="code-block" data-lang={n.lang ?? ''}><code>{n.code}</code></pre>
  {:else if n.type === 'query'}
    <pre class="code-block query-block" data-lang="query"><code>{n.text}</code></pre>
  {:else if n.type === 'quote'}
    <blockquote><Self nodes={n.children} {chain} /></blockquote>
  {:else if n.type === 'list'}
    <svelte:element this={n.ordered ? 'ol' : 'ul'}>
      {#each n.items as item, j (j)}
        <li><Self nodes={item} {chain} /></li>
      {/each}
    </svelte:element>
  {:else if n.type === 'table'}
    <table class="md-table">
      <thead>
        <tr>
          {#each n.header as cell, j (j)}
            <th style:text-align={n.aligns[j] === 'none' ? undefined : n.aligns[j]}><Inlines items={cell} {chain} /></th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each n.rows as row, r (r)}
          <tr>
            {#each row as cell, j (j)}
              <td style:text-align={n.aligns[j] === 'none' ? undefined : n.aligns[j]}><Inlines items={cell} {chain} /></td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if n.type === 'rule'}
    <hr />
  {/if}
{/each}

<style>
  p {
    margin: 0;
  }
  .md-heading {
    margin: 0.1em 0;
    font-weight: 650;
  }
  pre.code-block {
    margin: 0.2em 0;
    padding: 0.5em 0.7em;
    background: var(--code-bg);
    border-radius: 6px;
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 0.88em;
    white-space: pre;
  }
  .query-block {
    border-left: 3px solid var(--accent);
  }
  blockquote {
    margin: 0.2em 0;
    padding-left: 0.8em;
    border-left: 3px solid var(--border);
    color: var(--muted);
  }
  .md-table {
    border-collapse: collapse;
    margin: 0.3em 0;
  }
  .md-table th,
  .md-table td {
    border: 1px solid var(--border);
    padding: 0.2em 0.6em;
  }
  ul,
  ol {
    margin: 0.2em 0;
    padding-left: 1.4em;
  }
</style>
