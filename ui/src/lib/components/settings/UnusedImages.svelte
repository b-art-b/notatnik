<script lang="ts">
  // Settings → Graph: images below assets/ that no page mentions (ST-8). Looking is free; deleting moves the
  // files to the trash folder of this device, outside the graph.
  import { backend } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'
  import SettingRow from './SettingRow.svelte'

  type Item = { path: string; size: number }
  // `null` = not looked yet. $state makes the template re-run when the list is replaced.
  let items = $state<Item[] | null>(null)
  let busy = $state(false)

  const size = (b: number) =>
    b < 1024 * 1024 ? `${Math.max(1, Math.round(b / 1024))} KB` : `${(b / 1048576).toFixed(1)} MB`
  const total = $derived((items ?? []).reduce((n, i) => n + i.size, 0))

  async function look() {
    busy = true
    try {
      items = (await backend.call('list_orphan_assets', {})).items
    } catch (e) {
      toast(errorMessage(e), 'error')
    } finally {
      busy = false
    }
  }

  async function trash() {
    if (!items?.length) return
    busy = true
    try {
      const n = items.length
      items = (await backend.call('delete_orphan_assets', { paths: items.map((i) => i.path) })).items
      toast(`${n} unused image${n === 1 ? '' : 's'} moved to the trash`)
    } catch (e) {
      toast(errorMessage(e), 'error')
    } finally {
      busy = false
    }
  }
</script>

<SettingRow
  title="Unused images"
  description="Images in the assets folder that no page links, for example after you deleted the block that held them. Deleting keeps a copy in this device's trash for 30 days."
>
  <div class="unused">
    <button class="plain" data-testid="unused-look" disabled={busy} onclick={look}
      >Look for unused images</button
    >
    {#if items}
      {#if items.length === 0}
        <span data-testid="unused-none">None.</span>
      {:else}
        <span data-testid="unused-count">{items.length} unused, {size(total)}</span>
        <button class="plain" data-testid="unused-trash" disabled={busy} onclick={trash}>Delete them</button>
      {/if}
    {/if}
  </div>
  {#if items?.length}
    <ul class="list" data-testid="unused-list">
      {#each items as i (i.path)}
        <li><code>{i.path}</code> <span class="muted">{size(i.size)}</span></li>
      {/each}
    </ul>
  {/if}
</SettingRow>

<style>
  .unused {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .list {
    margin: 0.5rem 0 0;
    padding: 0;
    list-style: none;
    max-height: 10rem;
    overflow: auto;
    font-size: 0.85em;
  }
  .muted {
    color: var(--text-muted, inherit);
    opacity: 0.7;
  }
</style>
