<script lang="ts">
  // Settings → Graph: images below assets/ that no page mentions (ST-8). Looking is free; deleting moves the
  // files to the trash folder of this device, outside the graph. Each image has a preview and a copyable link.
  import { backend } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'
  import SettingRow from './SettingRow.svelte'

  type Item = { path: string; size: number }
  // `null` = not looked yet. $state makes the template re-run when the list is replaced.
  let items = $state<Item[] | null>(null)
  let busy = $state(false)
  /** The image shown large, by path. */
  let big = $state<string | null>(null)

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

  async function trash(paths: string[]) {
    if (paths.length === 0) return
    busy = true
    try {
      items = (await backend.call('delete_orphan_assets', { paths })).items
      big = null
      toast(`${paths.length} unused image${paths.length === 1 ? '' : 's'} moved to the trash`)
    } catch (e) {
      toast(errorMessage(e), 'error')
    } finally {
      busy = false
    }
  }

  /** Copies `![](assets/...)`. Pasted into a block it is rewritten to the path that suits that page. */
  async function copyLink(path: string) {
    try {
      await navigator.clipboard.writeText(`![](${path})`)
      toast('Link copied. Paste it into a block.')
    } catch {
      toast('Could not copy', 'error')
    }
  }
</script>

<SettingRow
  title="Unused images"
  description="Images in the assets folder that no page links, for example after you deleted the block that held them. Click a picture to enlarge it, copy its link to use it again, or delete it. Deleted images are kept in this device's trash folder."
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
        <button
          class="plain"
          data-testid="unused-trash"
          disabled={busy}
          onclick={() => trash(items!.map((i) => i.path))}>Delete all</button
        >
      {/if}
    {/if}
  </div>
</SettingRow>
{#if items?.length}
  <ul class="list" data-testid="unused-list">
    {#each items as i (i.path)}
      <li>
        <button
          type="button"
          class="thumb"
          class:big={big === i.path}
          aria-label={big === i.path ? 'Shrink' : 'Enlarge'}
          onclick={() => (big = big === i.path ? null : i.path)}
        >
          <img src={backend.assetUrl(i.path)} alt={i.path} loading="lazy" />
        </button>
        <div class="meta">
          <code>{i.path}</code>
          <span class="muted">{size(i.size)}</span>
          <span class="actions">
            <button type="button" class="plain" onclick={() => copyLink(i.path)}>Copy link</button>
            <button type="button" class="plain" disabled={busy} onclick={() => trash([i.path])}>Delete</button
            >
          </span>
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .unused {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .list {
    margin: 0.25rem 0 1rem;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  li {
    display: flex;
    gap: 0.8rem;
    align-items: flex-start;
  }
  .thumb {
    flex: none;
    padding: 0;
    border: 1px solid var(--border, #8884);
    border-radius: 4px;
    background: none;
    cursor: zoom-in;
    line-height: 0;
  }
  .thumb img {
    width: 96px;
    height: 72px;
    object-fit: contain;
  }
  .thumb.big {
    cursor: zoom-out;
  }
  .thumb.big img {
    width: min(560px, 60vw);
    height: auto;
    max-height: 70vh;
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85em;
    min-width: 0;
  }
  code {
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .muted {
    opacity: 0.7;
  }
</style>
