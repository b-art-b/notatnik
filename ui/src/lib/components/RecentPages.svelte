<script lang="ts">
  // "Recent": the pages you opened last, newest first, like Logseq's sidebar list.
  import { pageHashIn } from '../route'
  import { app } from '../stores/app.svelte'
  import { active } from '../stores/active.svelte'
  import { shownDate } from '../stores/format.svelte'
  import { padlockOf } from '../stores/encryption.svelte'
  import { useGraph } from '../graphContext'
  import { endPageDrag, startPageDrag } from '../transferDrag.svelte'
  import Padlock from './Padlock.svelte'

  // the graph of this section: its own pages, icons, locks; the page on screen is marked only when it is this graph's
  const g = useGraph()
  const SHOWN = 10
  const current = $derived(g === active() && app.route.kind === 'page' ? app.route.name.toLowerCase() : '')
  const isDate = (n: string) => /^\d{4}-\d{2}-\d{2}$/.test(n)
  import PageIcon from './PageIcon.svelte'

  // the lock: which of the listed pages are stored encrypted (the file is covered by a marked path, EN-12).
  // Recent keeps names only, so the files are asked for.
  let files = $state<Record<string, string | null>>({})
  $effect(() => {
    void g.encryption.refresh()
  })
  $effect(() => {
    void g.bus.version
    const names = g.recent.items.slice(0, SHOWN + 5)
    let cancelled = false
    void g.backend.call('page_files', { names }).then((r) => {
      if (!cancelled) files = Object.fromEntries(names.map((n, i) => [n.toLowerCase(), r.files[i] ?? null]))
    })
    return () => {
      cancelled = true
    }
  })
  const padlock = (name: string) => padlockOf(g.encryption.status, files[name.toLowerCase()] ?? '')
</script>

{#if g.recent.items.length > 0}
  <section class="recent" aria-label="Recent pages" data-testid="recent-pages">
    <div class="head">
      <h3>Recent</h3>
      <button class="clear" onclick={() => g.recent.clear()} title="Clear the list">clear</button>
    </div>
    <ul>
      {#each (void g.archived.names, g.recent.items.filter((n) => !g.archived.is(n))).slice(0, SHOWN) as name (name.toLowerCase())}
        <li>
          <a
            href={pageHashIn(g.slot, name)}
            class="page-link"
            data-recent={name}
            draggable="true"
            ondragstart={(e) => startPageDrag(e, g.slot, name)}
            ondragend={endPageDrag}
            aria-current={current === name.toLowerCase() ? 'page' : undefined}
            class:on={current === name.toLowerCase()}
            title={name}><PageIcon {name} align />{isDate(name) ? shownDate(name) : name}</a
          ><Padlock mark={padlock(name)} testid="recent-lock" gap="0.3em" />
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
