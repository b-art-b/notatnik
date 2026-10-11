<script lang="ts">
  // "Recent": the pages you opened last, newest first, like Logseq's sidebar list.
  import { pageHash } from '../route'
  import { app } from '../stores/app.svelte'
  import { clearRecent, recent } from '../stores/recent.svelte'
  import { archived, isArchived } from '../stores/archived.svelte'
  import { shownDate } from '../stores/format.svelte'

  const SHOWN = 10
  const current = $derived(app.route.kind === 'page' ? app.route.name.toLowerCase() : '')
  const isDate = (n: string) => /^\d{4}-\d{2}-\d{2}$/.test(n)
  import PageIcon from './PageIcon.svelte'
</script>

{#if recent.items.length > 0}
  <section class="recent" aria-label="Recent pages" data-testid="recent-pages">
    <div class="head">
      <h3>Recent</h3>
      <button class="clear" onclick={clearRecent} title="Clear the list">clear</button>
    </div>
    <ul>
      {#each (void archived.names, recent.items.filter((n) => !isArchived(n))).slice(0, SHOWN) as name (name.toLowerCase())}
        <li>
          <a
            href={pageHash(name)}
            data-recent={name}
            aria-current={current === name.toLowerCase() ? 'page' : undefined}
            class:on={current === name.toLowerCase()}
            title={name}><PageIcon {name} />{isDate(name) ? shownDate(name) : name}</a
          >
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  h3 {
    margin: 0.6em 0 0.3em;
  }
  .clear {
    border: none;
    background: none;
    color: var(--muted);
    font-size: 0.75em;
    cursor: pointer;
    padding: 0;
  }
  .clear:hover {
    color: var(--link);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    padding: 0.1em 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  a {
    text-decoration: none;
  }
  a.on {
    font-weight: 600;
  }
</style>
