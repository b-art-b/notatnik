<script lang="ts">
  // Settings → Linked files: the servers the `/file` picker can browse. The open graph's own server is always
  // there; others are kept on this device, with the password in the system's keyring.
  import { onMount } from 'svelte'
  import { backend, type FileSourceView } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'
  import FileSourceForm from '../FileSourceForm.svelte'
  import { documentLinkSetting, setDocumentLinkSetting } from '../../annotate/remote'
  import { dockSetting, setDockSetting } from '../../stores/dock.svelte'
  import Switch from '../Switch.svelte'

  let sources = $state<FileSourceView[]>([])
  // what a click on a link to a PDF or an EPUB opens on this device (LV-2)
  let opens = $state(documentLinkSetting())
  // documents open in the side panel beside the notes, not full screen (DD-2)
  let docked = $state(dockSetting())
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

<fieldset class="opens" data-testid="document-links">
  <legend>Open PDF and EPUB links in</legend>
  <label
    ><input
      type="radio"
      name="document-links"
      value="viewer"
      checked={opens === 'viewer'}
      onchange={() => setDocumentLinkSetting((opens = 'viewer'))}
    /> This app's viewer (you can highlight and comment)</label
  >
  <label
    ><input
      type="radio"
      name="document-links"
      value="system"
      checked={opens === 'system'}
      onchange={() => setDocumentLinkSetting((opens = 'system'))}
    /> The system's program</label
  >
  <small class="note"
    >On this device. Ctrl+click (Cmd+click, middle click) or the ↗ after a link opens the other one. Files of
    this computer open in the viewer in the desktop app only.</small
  >
</fieldset>

<div class="opens docking">
  <label
    ><Switch
      checked={docked}
      aria-label="Open documents beside the notes"
      data-testid="dock-setting"
      onchange={(e) => setDockSetting((docked = e.currentTarget.checked))}
    /> Open documents beside the notes, in the side panel</label
  >
  <small class="note"
    >On this device, off by default. A PDF or EPUB then opens docked next to the page (when the window is at
    least 900 px wide); <em>Full screen</em> in its header brings it back. Without it a document opens full
    screen and <em>Dock</em> moves it.</small
  >
</div>

<p class="downloads">
  <button type="button" class="plain" data-testid="clear-downloads" onclick={clearDownloads}
    >Delete downloaded documents</button
  >
</p>

<style>
  .opens {
    margin: 1em 0 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.5em 0.9em;
  }
  .docking {
    border: 1px solid var(--border);
    border-radius: 8px;
    margin-top: 0.8em;
  }
  .opens label {
    display: block;
    padding: 0.15em 0;
  }
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
