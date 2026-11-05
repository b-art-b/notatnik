<script lang="ts">
  // Settings → Linked files: the servers the `/file` picker can browse. The open graph's own server is always
  // there; others are kept on this device, with the password in the system's keyring.
  import { onMount } from 'svelte'
  import { backend, type FileSourceView } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'
  import FileSourceForm from '../FileSourceForm.svelte'

  let sources = $state<FileSourceView[]>([])
  /** `new`, a source ID being edited, or nothing. */
  let editing = $state<string | null>(null)
  let removing = $state<string | null>(null)

  async function load() {
    try {
      sources = await backend.call('file_sources', {})
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  onMount(load)

  async function remove(id: string) {
    try {
      sources = await backend.call('file_source_delete', { id })
      removing = null
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  async function clearDownloads() {
    try {
      const r = await backend.call('remote_cache_clear', {})
      toast(`Deleted the downloaded documents (${Math.round(Number(r.freed) / 1024 / 1024)} MB).`)
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  import PaneHeader from './PaneHeader.svelte'
</script>

<PaneHeader title="Linked files" scope="device">
  Servers whose files you can link from a note: type <kbd>/file</kbd> in a block and choose one. A link to a
  Nextcloud file is its <code>…/f/&lt;number&gt;</code> address, which keeps working when the file is renamed or
  moved. A link to a PDF or an EPUB on the server this graph is synced with opens in the app's viewer; the file
  is downloaded to this device (not into the graph) and kept up to 500 MB.
</PaneHeader>

<ul class="sources" aria-label="Servers" data-testid="file-sources">
  {#each sources as s (s.id)}
    <li>
      {#if editing === s.id}
        <FileSourceForm
          source={s}
          onsaved={() => {
            editing = null
            void load()
          }}
          oncancel={() => (editing = null)}
        />
      {:else}
        <div class="head">
          <div>
            <strong>{s.name}</strong>
            <div class="sub">{s.user} · {s.server}</div>
          </div>
          {#if s.graph}
            <span class="note">The server this graph is synced with</span>
          {:else if removing === s.id}
            <span class="buttons">
              <button type="button" onclick={() => remove(s.id)}>Remove</button>
              <button type="button" class="plain" onclick={() => (removing = null)}>Cancel</button>
            </span>
          {:else}
            <span class="buttons">
              <button type="button" class="plain" onclick={() => (editing = s.id)}>Edit</button>
              <button type="button" class="plain" onclick={() => (removing = s.id)}>Remove…</button>
            </span>
          {/if}
        </div>
      {/if}
    </li>
  {:else}
    <li class="none">No servers yet.</li>
  {/each}
</ul>

{#if editing === 'new'}
  <div class="box">
    <FileSourceForm
      onsaved={() => {
        editing = null
        void load()
      }}
      oncancel={() => (editing = null)}
    />
  </div>
{:else}
  <button type="button" class="plain" data-testid="file-source-add" onclick={() => (editing = 'new')}
    >Add a server…</button
  >
{/if}

<p class="downloads">
  <button type="button" class="plain" data-testid="clear-downloads" onclick={clearDownloads}
    >Delete downloaded documents</button
  >
</p>

<style>
  .downloads {
    margin-top: 1em;
  }
  .sources {
    list-style: none;
    margin: 0.6em 0;
    padding: 0;
  }
  .sources li {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.6em 0.9em;
    margin-bottom: 0.5em;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 0.5em;
  }
  .sub,
  .note,
  .none {
    color: var(--muted);
    font-size: 0.85em;
    word-break: break-all;
  }
  .none {
    border: none;
  }
  .buttons {
    display: flex;
    gap: 0.4em;
  }
  .box {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.8em 1em;
  }
</style>
