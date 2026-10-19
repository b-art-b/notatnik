<script lang="ts">
  // Deleting a page: the file moves to the trash (outside the graph folder, kept 30 days) and the toast offers Undo.
  // Links to the page stay; they then point to an empty page.
  import { backend, type TitleHit } from '../api'
  import { app, errorMessage, navigate, toast } from '../stores/app.svelte'
  import { openDocs } from '../stores/pages.svelte'
  import { removeRecent } from '../stores/recent.svelte'

  // `stay`: opened from a list (All pages): the list stays, nothing navigates away
  let { page, onclose, stay = false }: { page: string; onclose: () => void; stay?: boolean } = $props()
  let subpages = $state<TitleHit[]>([])
  let linked = $state<number | null>(null)
  let withSub = $state(false)
  let error = $state('')
  let busy = $state(false)

  // what the user is about to lose track of: pages below it and pages that link to it
  $effect(() => {
    let cancelled = false
    void backend.call('subpages', { page }).then((r) => {
      if (!cancelled) subpages = r.filter((p) => !p.is_virtual)
    })
    void backend.call('backlinks', { page }).then((r) => {
      if (!cancelled) linked = new Set(r.map((l) => l.source_page)).size
    })
    return () => {
      cancelled = true
    }
  })

  async function remove() {
    busy = true
    error = ''
    try {
      const r = await backend.call('delete_pages', { page, include_subpages: withSub })
      for (const n of r.deleted) removeRecent(n)
      // open documents of the deleted pages are stale now
      for (const d of openDocs()) if (r.deleted.includes(d.name)) void d.load()
      onclose()
      if (!stay) navigate({ kind: 'journals' })
      const what = r.deleted.length === 1 ? `“${r.deleted[0]}”` : `${r.deleted.length} pages`
      toast(`Deleted ${what}`, 'info', {
        label: 'Undo',
        run: async () => {
          try {
            const back = await backend.call('restore_pages', { trash_id: r.trash_id })
            toast(`Restored ${back.items.length === 1 ? `“${back.items[0]}”` : `${back.items.length} pages`}`)
            if (back.items[0] && !stay) navigate({ kind: 'page', name: back.items[0], zoom: [] })
          } catch (e) {
            toast(errorMessage(e), 'error')
          }
        },
      })
      void app
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }
</script>

<div class="dialog" role="dialog" aria-label="Delete page" aria-modal="true">
  <p>
    Delete <strong>{page}</strong>? The file moves to the trash and can be restored for 30 days (Undo in the
    message that follows).
  </p>
  {#if linked}
    <p class="warn">
      {linked} page{linked === 1 ? '' : 's'} link{linked === 1 ? 's' : ''} to it. The links stay and will point
      to an empty page.
    </p>
  {/if}
  {#if subpages.length}
    <label>
      <input type="checkbox" bind:checked={withSub} />
      Also delete the {subpages.length} page{subpages.length === 1 ? '' : 's'} below it
    </label>
    {#if !withSub}<p class="muted">They stay where they are.</p>{/if}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <button class="danger" disabled={busy} onclick={remove}>Delete</button>
  <button onclick={onclose}>Cancel</button>
</div>

<style>
  .dialog {
    border: 1px solid var(--border);
    background: var(--panel);
    padding: 0.8em;
    border-radius: 8px;
    margin: 0.5em 0;
  }
  .warn {
    color: var(--warn);
  }
  .error {
    color: var(--danger);
  }
  .danger {
    color: var(--danger);
    font-weight: 600;
  }
</style>
