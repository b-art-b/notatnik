<script lang="ts">
  // Page history (VH-1..VH-3): the recorded states of this page's file, what changed against the file now, and a
  // button to put one back. Restoring is itself recorded, so it can be undone from this list.
  import { backend, type PageVersionView } from '../api'
  import { errorMessage, toast } from '../stores/app.svelte'
  import { shownDate } from '../stores/format.svelte'
  import { openDocs } from '../stores/pages.svelte'
  import { changed, diffLines } from '../diff'

  let { page, onclose }: { page: string; onclose: () => void } = $props()
  let versions = $state<PageVersionView[]>([])
  let loaded = $state(false)
  let selected = $state<string | null>(null)
  let current = $state('')
  let text = $state('')
  let busy = $state(false)

  function when(ms: number): string {
    const d = new Date(ms)
    const iso = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    return `${shownDate(iso)}, ${time}`
  }
  const size = (b: number) => (b < 1024 ? `${b} B` : `${(b / 1024).toFixed(1)} KB`)

  async function load() {
    versions = await backend.call('page_versions', { page })
    loaded = true
  }
  $effect(() => {
    void load().catch((e) => toast(errorMessage(e), 'error'))
  })

  async function select(id: string) {
    selected = id
    try {
      const [v, c] = await Promise.all([
        backend.call('page_version_text', { page, id }),
        backend.call('page_version_text', { page, id: 'current' }),
      ])
      text = v.value
      current = c.value
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  // what restoring would change: the version is the "after", the file now the "before"
  const diff = $derived(selected ? diffLines(current, text) : [])

  async function restore() {
    if (!selected) return
    busy = true
    try {
      await backend.call('restore_page_version', { page, id: selected })
      for (const d of openDocs()) if (d.name === page) void d.load()
      toast('Page restored. The text it replaced is in the history.')
      selected = null
      await load()
    } catch (e) {
      toast(errorMessage(e), 'error')
    } finally {
      busy = false
    }
  }
</script>

<div class="dialog" role="dialog" aria-label="Page history" data-testid="history-dialog">
  <p>
    <strong>History of {page}</strong>
    <span class="muted"
      >Versions of this page's file recorded on this device (not synced), thinned out over time.</span
    >
  </p>
  {#if loaded && versions.length === 0}
    <p class="muted">No earlier versions yet. They are recorded when the page changes.</p>
  {/if}
  <div class="cols">
    <ul class="list" aria-label="Versions">
      {#each versions as v (v.id)}
        <li>
          <button
            class:on={selected === v.id}
            aria-pressed={selected === v.id}
            data-testid="history-version"
            onclick={() => select(v.id)}>{when(v.at_ms)} <span class="muted">{size(v.bytes)}</span></button
          >
        </li>
      {/each}
    </ul>
    {#if selected}
      <div class="preview">
        {#if !changed(diff)}
          <p class="muted">Same as the page now.</p>
        {:else}
          <p class="muted">Restoring changes the page like this (− now, + this version):</p>
          <pre class="diff" data-testid="history-diff">{#each diff as l, i (i)}<span
                class={l.kind}
                data-kind={l.kind}
                >{l.kind === 'add' ? '+ ' : l.kind === 'del' ? '− ' : '  '}{l.text}
</span>{/each}</pre>
        {/if}
        <button disabled={busy || !changed(diff)} onclick={restore} data-testid="history-restore"
          >Restore this version</button
        >
      </div>
    {/if}
  </div>
  <button onclick={onclose}>Close</button>
</div>

<style>
  .dialog {
    border: 1px solid var(--border);
    background: var(--panel);
    padding: 0.8em;
    border-radius: 8px;
    margin: 0.5em 0;
  }
  .cols {
    display: flex;
    gap: 1em;
    align-items: flex-start;
    flex-wrap: wrap;
    margin: 0.5em 0;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 18em;
    overflow: auto;
    min-width: 14em;
  }
  .list button {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    color: inherit;
    padding: 0.25em 0.5em;
    border-radius: 6px;
    cursor: pointer;
  }
  .list button:hover {
    background: var(--chip);
  }
  .list button.on {
    background: var(--selection);
    font-weight: 600;
  }
  .preview {
    flex: 1 1 20em;
    min-width: 0;
  }
  .diff {
    max-height: 18em;
    overflow: auto;
    margin: 0.3em 0 0.6em;
    padding: 0.4em;
    border: 1px solid var(--border);
    border-radius: 6px;
    font-size: 0.85em;
    white-space: pre-wrap;
  }
  .diff span {
    display: block;
  }
  .add {
    background: color-mix(in srgb, #2e9d4f 18%, transparent);
  }
  .del {
    background: color-mix(in srgb, var(--danger) 18%, transparent);
  }
  .muted {
    color: var(--muted);
    font-size: 0.85em;
  }
</style>
