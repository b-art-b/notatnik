<script lang="ts">
  import type { Node } from '../api'
  import Inlines from './Inlines.svelte'
  import Self from './Nodes.svelte'
  import QueryBlock from './QueryBlock.svelte'
  import MermaidDiagram from './MermaidDiagram.svelte'

  let { nodes, chain = [] }: { nodes: Node[]; chain?: string[] } = $props()
</script>

{#each nodes as n, i (i)}
  {#if n.type === 'paragraph'}
    <p><Inlines items={n.children} {chain} /></p>
  {:else if n.type === 'heading'}
    <svelte:element this={`h${Math.min(n.level, 6)}`} class="md-heading"
      ><Inlines items={n.children} {chain} /></svelte:element
    >
  {:else if n.type === 'code_block' && n.lang === 'mermaid'}
    <MermaidDiagram code={n.code} />
  {:else if n.type === 'code_block'}
    <pre class="code-block" data-lang={n.lang ?? ''}><code>{n.code}</code></pre>
  {:else if n.type === 'query'}
    <QueryBlock text={n.text} />
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
            <th style:text-align={n.aligns[j] === 'none' ? undefined : n.aligns[j]}
              ><Inlines items={cell} {chain} /></th
            >
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each n.rows as row, r (r)}
          <tr>
            {#each row as cell, j (j)}
              <td style:text-align={n.aligns[j] === 'none' ? undefined : n.aligns[j]}
                ><Inlines items={cell} {chain} /></td
              >
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if n.type === 'callout'}
    <!-- a div with a role: the kind is text from the note, so it is only used as a data attribute -->
    <div class="callout" data-kind={n.kind} role="note">
      <Self nodes={n.children} {chain} />
    </div>
  {:else if n.type === 'details'}
    <details open={n.open}>
      <!-- the click opens or closes the section; it must not also start editing the block -->
      <summary onclick={(e) => e.stopPropagation()}
        >{#if n.summary.length}<Inlines items={n.summary} {chain} />{:else}Details{/if}</summary
      >
      <Self nodes={n.children} {chain} />
    </details>
  {:else if n.type === 'footnote'}
    <div class="footnote" data-label={n.label}>
      <sup>{n.label}</sup>
      <div class="footnote-body"><Self nodes={n.children} {chain} /></div>
    </div>
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
  .callout {
    margin: 0.3em 0;
    padding: 0.4em 0.8em;
    border-left: 4px solid var(--accent);
    background: var(--code-bg);
    border-radius: 4px;
  }
  .callout[data-kind='warning'],
  .callout[data-kind='caution'] {
    border-left-color: #d9a000;
  }
  .callout[data-kind='danger'],
  .callout[data-kind='error'] {
    border-left-color: #d64545;
  }
  .callout[data-kind='success'],
  .callout[data-kind='tip'] {
    border-left-color: #2f9e5b;
  }
  details {
    margin: 0.2em 0;
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  .footnote {
    display: flex;
    gap: 0.5em;
    margin: 0.2em 0;
    font-size: 0.9em;
    color: var(--muted);
  }
  .footnote-body {
    min-width: 0;
  }
  ul,
  ol {
    margin: 0.2em 0;
    padding-left: 1.4em;
  }
</style>
