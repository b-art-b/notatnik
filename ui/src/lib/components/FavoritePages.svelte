<script lang="ts">
  // "Favorites": the pages you pinned, in the order you chose, above "Recent" in the sidebar. The list is a setting of
  // the graph, so it is the same on every device. Drag a row, or press Alt+↑ / Alt+↓ on it, to move it.
  import { pageHashIn } from '../route'
  import { app } from '../stores/app.svelte'
  import { active } from '../stores/active.svelte'
  import { padlockOf } from '../stores/encryption.svelte'
  import { useGraph } from '../graphContext'
  import { endPageDrag, startPageDrag } from '../transferDrag.svelte'
  import { shownDate } from '../stores/format.svelte'
  import PageIcon from './PageIcon.svelte'
  import Padlock from './Padlock.svelte'

  // the graph of this section; the page on screen is marked only when it is this graph's
  const g = useGraph()
  const favorites = $derived(g.favorites)
  const current = $derived(g === active() && app.route.kind === 'page' ? app.route.name.toLowerCase() : '')
  const isDate = (n: string) => /^\d{4}-\d{2}-\d{2}$/.test(n)

  // read again when anything changes: a rename or a delete changes the list, and so does a sync that brought the
  // settings of another device
  $effect(() => {
    void g.bus.version
    void g.favorites.load()
  })
  $effect(() => {
    void g.encryption.refresh()
  })

  // dragging: the name being dragged and the row it is over
  let dragging = $state<string | null>(null)
  let over = $state<number | null>(null)

  function drop(to: number) {
    const name = dragging
    dragging = over = null
    if (name !== null) void g.favorites.move(name, to)
  }
  function onkeydown(e: KeyboardEvent, name: string, i: number) {
    if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return
    e.preventDefault()
    const to = Math.max(0, Math.min(favorites.items.length - 1, i + (e.key === 'ArrowUp' ? -1 : 1)))
    if (to === i) return
    void g.favorites.move(name, to).then(() => {
      // the row is drawn again in its new place: give it the focus back so the next key press goes on
      queueMicrotask(() =>
        document.querySelector<HTMLElement>(`[data-favorite="${CSS.escape(name)}"]`)?.focus(),
      )
    })
  }
</script>

{#if favorites.items.length}
  <section class="favorites" aria-label="Favorites" data-testid="favorites">
    <h3>Favorites</h3>
    <ul>
      {#each favorites.items as f, i (f.name.toLowerCase())}
        <li
          class:over={over === i && dragging !== null && dragging !== f.name}
          draggable="true"
          ondragstart={(e) => {
            dragging = f.name
            e.dataTransfer?.setData('text/plain', f.name)
            // dragged to the other graph it is a page to take there
            startPageDrag(e, g.slot, f.name)
          }}
          ondragover={(e) => {
            if (dragging === null) return
            e.preventDefault()
            over = i
          }}
          ondragleave={() => (over = null)}
          ondrop={(e) => {
            e.preventDefault()
            drop(i)
          }}
          ondragend={() => {
            dragging = over = null
            endPageDrag()
          }}
        >
          <a
            href={pageHashIn(g.slot, f.name)}
            class="page-link"
            data-favorite={f.name}
            aria-current={current === f.name.toLowerCase() ? 'page' : undefined}
            class:on={current === f.name.toLowerCase()}
            title={`${f.name} (Alt+↑ and Alt+↓ move it)`}
            onkeydown={(e) => onkeydown(e, f.name, i)}
            ><PageIcon name={f.name} align />{isDate(f.name) ? shownDate(f.name) : f.name}</a
          ><Padlock
            mark={padlockOf(g.encryption.status, f.path ?? '')}
            testid="favorite-lock"
            gap="0.3em"
          /><button
            type="button"
            class="remove"
            aria-label={`Remove ${f.name} from the favorites`}
            title="Remove from the favorites"
            onclick={() => g.favorites.set(f.name, false)}>×</button
          >
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  h3 {
    margin: 0.6em 0 0.3em;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: baseline;
    padding: 0.1em 0;
    border-top: 2px solid transparent;
    white-space: nowrap;
  }
  /* where a dragged row would land */
  li.over {
    border-top-color: var(--accent);
  }
  li a {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    text-decoration: none;
  }
  li a.on {
    font-weight: 600;
  }
  /* the remove button is there for the keyboard and the pointer, quiet until the row is looked at */
  .remove {
    margin-left: auto;
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0 0.3em;
    opacity: 0;
  }
  li:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }
  .remove:hover {
    color: var(--danger);
  }
</style>
