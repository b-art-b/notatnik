<script lang="ts">
  // "Scheduled / due today" above today's journal. Read from the agenda queries; never written into the file.
  import { backend, type AgendaItem } from '../api'
  import { bus } from '../stores/events.svelte'
  import Block from './Block.svelte'

  let items = $state<AgendaItem[]>([])
  $effect(() => {
    void bus.version
    let cancelled = false
    backend.call('agenda', { ns: null, tag: null, priority: null }).then((a) => {
      if (!cancelled) items = a.today_items
    })
    return () => {
      cancelled = true
    }
  })
</script>

{#if items.length}
  <section class="today" aria-label="Scheduled and due today" data-testid="today-tasks">
    <h2>Scheduled / due today</h2>
    {#each items as it (it.page + it.block.id)}
      <div class="crumbs">{it.page}</div>
      <Block block={it.block} docName={it.page} viewKey={`today-${it.page}-${it.block.id}`} />
    {/each}
  </section>
{/if}

<style>
  .today {
    border-left: 3px solid var(--accent);
    padding-left: 0.6em;
    margin-bottom: 1em;
  }
  h2 {
    font-size: 0.9em;
    color: var(--muted);
    margin: 0 0 0.3em;
  }
  .crumbs {
    font-size: 0.72em;
    color: var(--muted);
    margin-left: 34px;
  }
</style>
