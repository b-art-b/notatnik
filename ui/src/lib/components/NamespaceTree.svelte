<script lang="ts">
  import type { NamespaceEntry } from '../api'
  import { pageHashIn } from '../route'
  import Self from './NamespaceTree.svelte'
  import { padlockOf } from '../stores/encryption.svelte'
  import { useGraph } from '../graphContext'
  import { endPageDrag, startPageDrag } from '../transferDrag.svelte'
  import Padlock from './Padlock.svelte'
  import { toggleArchive } from '../pageActions'
  import DeleteDialog from './DeleteDialog.svelte'

  // `actions`: All pages shows Archive / Delete… on each page; the sidebar tree does not
  // `lock`: shows the padlock of a page that differs from the graph's default (All pages and the sidebar)
  // `only`: names to keep (lowercase): All pages with the padlock filter; every branch is then open
  let {
    parent = null,
    actions = false,
    lock = false,
    only = null,
  }: { parent?: string | null; actions?: boolean; lock?: boolean; only?: Set<string> | null } = $props()
  // the graph the tree lists: its section's, or (All pages) the active one
  const g = useGraph()
  let deleting = $state('') // the page whose Delete… confirmation is open
  const padlock = (e: NamespaceEntry) =>
    actions || lock ? padlockOf(g.encryption.status, e.path ?? '') : null
  $effect(() => {
    if (actions || lock) void g.encryption.refresh()
  })
  let all = $state<NamespaceEntry[]>([])
  // archived pages (and what is below them) are not in the tree
  const entries = $derived(
    (void g.archived.names,
    all.filter((e) => !g.archived.is(e.name) && (!only || only.has(e.name.toLowerCase())))),
  )
  // What was opened or closed by hand; every other branch follows `ns.all` (Collapse all / Expand all).
  // The state is shared by all levels of the tree, so it is read once and saved by whichever level changes it.

  $effect(() => {
    void g.bus.version
    let cancelled = false
    g.backend.call('list_namespace', { parent }).then((r) => {
      if (!cancelled) all = r
    })
    return () => {
      cancelled = true
    }
  })
  const tree = g.tree
  const isOpen = (name: string) => !!only || (tree.open[name] ?? tree.all)
  function toggle(name: string) {
    tree.open[name] = !isOpen(name)
    tree.save()
  }
  // the root level answers the buttons of the panel (the requests made before this tree existed are not repeated)
  let seen = g.tree.foldN
  $effect(() => {
    if (parent !== null || tree.foldN === seen) return
    seen = tree.foldN
    tree.all = !tree.foldCollapsed
    tree.open = {}
    tree.save()
  })
  import PageIcon from './PageIcon.svelte'
</script>

<ul
  role={parent === null ? 'tree' : 'group'}
  class="tree"
  aria-label={parent === null ? 'Namespaces' : undefined}
>
  {#each entries as e (e.name)}
    <li role="treeitem" aria-selected="false" aria-expanded={e.has_children ? isOpen(e.name) : undefined}>
      <span class="line">
        {#if e.has_children}<button
            class="tog"
            aria-label={isOpen(e.name) ? 'Collapse' : 'Expand'}
            onclick={() => toggle(e.name)}>{isOpen(e.name) ? '▾' : '▸'}</button
          >{:else}<span class="tog"></span>{/if}
        <a
          href={pageHashIn(g.slot, e.name)}
          class:virtual={e.is_virtual}
          data-ns={e.name}
          draggable={!e.is_virtual}
          ondragstart={(ev) => startPageDrag(ev, g.slot, e.name, e.descendants > 0)}
          ondragend={endPageDrag}><PageIcon name={e.name} align />{e.leaf}</a
        >
        <Padlock mark={padlock(e)} testid="row-lock" />
        {#if e.descendants}<span class="count">{e.descendants}</span>{/if}
        <!-- a page without a file has nothing to archive or delete -->
        {#if actions && !e.is_virtual}
          <span class="acts">
            <button
              class="act"
              data-testid="row-archive"
              title="Hide this page (and the pages below it) from the sidebar and the agenda"
              onclick={() => toggleArchive(e.name)}>Archive</button
            >
            <button
              class="act danger"
              data-testid="row-delete"
              aria-label={`Delete ${e.name}`}
              onclick={() => (deleting = deleting === e.name ? '' : e.name)}>Delete…</button
            >
          </span>
        {/if}
      </span>
      {#if actions && deleting === e.name}
        <DeleteDialog page={e.name} stay onclose={() => (deleting = '')} />
      {/if}
      {#if e.has_children && isOpen(e.name)}<Self parent={e.name} {actions} {lock} {only} />{/if}
    </li>
  {/each}
</ul>

<style>
  .tree {
    list-style: none;
    margin: 0;
    padding-left: 0.9em;
  }
  .line {
    display: flex;
    gap: 2px;
    align-items: center;
  }
  .tog {
    width: 14px;
    border: none;
    background: none;
    color: var(--muted);
    padding: 0;
    cursor: pointer;
  }
  a {
    color: var(--text);
    text-decoration: none;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .virtual {
    color: var(--muted);
    font-style: italic;
  }
  .count {
    font-size: 0.7em;
    color: var(--muted);
  }
  .acts {
    display: inline-flex;
    gap: 0.2em;
    margin-left: 0.6em;
    white-space: nowrap;
  }
  .act {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0.1em 0.4em;
    font: inherit;
    font-size: 0.8em;
  }
  .act:hover,
  .act:focus-visible {
    color: var(--accent);
    background: var(--chip);
    border-radius: 4px;
  }
  .act.danger:hover,
  .act.danger:focus-visible {
    color: var(--danger);
  }
</style>
