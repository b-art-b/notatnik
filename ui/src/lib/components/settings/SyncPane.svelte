<script lang="ts">
  // Settings → Sync: the server folder of the open graph and how the sync is doing. A graph that is only a local
  // folder shows how to get one on a server. The sync itself runs in the backend (see stores/sync.svelte.ts).
  import { backend } from '../../api'
  import { app, errorMessage, adoptGraph, toast } from '../../stores/app.svelte'
  import { refreshSync, sync, syncLine, syncNow } from '../../stores/sync.svelte'
  import SettingRow from './SettingRow.svelte'
  import EncryptionSection from './EncryptionSection.svelte'

  let password = $state('')
  let confirmDisconnect = $state(false)
  const st = $derived(sync.status)

  async function signIn() {
    try {
      sync.status = await backend.call('sync_set_password', { password })
      password = ''
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  async function disconnect() {
    try {
      await adoptGraph(await backend.call('disconnect_remote', {}))
      confirmDisconnect = false
      await refreshSync()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
</script>

<h2>Sync</h2>
{#if app.graph?.remote && st}
  <p class="scope">
    <span class="badge">This device</span>
    This graph is kept in a folder on a server. This device holds a full copy and works without a connection; the
    two are brought in step in the background.
  </p>
  <SettingRow
    title="Server folder"
    description="The WebDAV folder of this graph. Files are stored as they are, except the ones you choose to encrypt (see Encryption below)."
  >
    <code data-testid="sync-url">{st.url}</code>
  </SettingRow>
  <SettingRow title="Login" description="The password is kept in the system's keyring, never in a file.">
    <span data-testid="sync-user">{st.user}</span>
  </SettingRow>
  <SettingRow
    title="Merge changes from two devices"
    description="When the same page was changed here and on the server, combine the changes block by block if they do not overlap. Off, or when they do overlap, your version is kept next to the server's as a conflicted copy that you resolve under the conflict warning."
    labelFor="sync-merge"
  >
    <input
      id="sync-merge"
      class="switch"
      type="checkbox"
      role="switch"
      checked={st.merge}
      onchange={async (e) => {
        try {
          sync.status = await backend.call('sync_set_merge', {
            enabled: (e.currentTarget as HTMLInputElement).checked,
          })
        } catch (err) {
          toast(errorMessage(err), 'error')
        }
      }}
    />
  </SettingRow>
  <SettingRow
    title="Status"
    description="Changes here are sent a few seconds after you stop typing, changes elsewhere arrive within a minute, and the app syncs once more when it closes."
  >
    <div class="status">
      <strong data-testid="sync-line">{syncLine(st)}</strong>
      <button
        type="button"
        class="plain"
        disabled={sync.busy || st.state === 'syncing'}
        onclick={() => syncNow()}>Sync now</button
      >
    </div>
  </SettingRow>
  {#if st.last_sync}
    <p class="last" data-testid="sync-last">
      Last complete sync {new Date(st.last_sync).toLocaleString()}: {st.pushed} sent, {st.pulled} received,
      {st.deleted} deleted{st.conflicts ? `, ${st.conflicts} conflict(s)` : ''}.
    </p>
  {/if}
  {#if st.conflicts}
    <p class="warn" role="note">
      The same page was changed on two devices. The server's version stays; your version is saved next to it
      as "… (conflicted copy …)" and shown as a conflict in the page list.
    </p>
  {/if}

  {#if st.error_code === 'auth'}
    <form
      class="box"
      onsubmit={(e) => {
        e.preventDefault()
        void signIn()
      }}
    >
      <p role="alert">{st.error}</p>
      <label for="sync-pw">Password for {st.user} (on Nextcloud: an app password)</label>
      <input id="sync-pw" type="password" autocomplete="current-password" bind:value={password} />
      <button type="submit" disabled={!password}>Sign in</button>
    </form>
  {:else if st.error_code === 'mass_delete'}
    <div class="box" role="alert">
      <p>{st.error}</p>
      <p>
        If you did delete that many pages on purpose, the sync can go ahead. The deleted files are moved to
        <code>.notatnik/trash</code> here, and the server keeps them in its own trash.
      </p>
      <button type="button" onclick={() => syncNow(true)}>Delete {st.pending_deletes} files and sync</button>
    </div>
  {:else if st.error}
    <p class="warn" role="alert" data-testid="sync-error">{st.error}</p>
  {/if}
  {#if st.warnings.length}
    <div class="box" role="note" data-testid="sync-warnings">
      <p>
        {st.warnings.length === 1 ? 'This file was' : 'These files were'} downloaded with the lines of its blocks
        joined together, which another editor does when it saves a note (Nextcloud's Text editor, for one). Properties
        such as <code>deadline::</code> then stop working. Open the page, choose <strong>History</strong> and restore
        the version from before.
      </p>
      <ul>
        {#each st.warnings as w (w)}<li><code>{w}</code></li>{/each}
      </ul>
    </div>
  {/if}
  {#if st.file_errors.length}
    <ul class="errors" aria-label="Files that failed">
      {#each st.file_errors as f, i (i)}<li>{f}</li>{/each}
    </ul>
  {/if}

  <EncryptionSection />

  <SettingRow
    title="Disconnect"
    description="Keeps the notes on this device as a graph without a server and forgets the password. The server folder is not touched."
  >
    {#if confirmDisconnect}
      <button type="button" onclick={disconnect}>Disconnect</button>
      <button type="button" class="plain" onclick={() => (confirmDisconnect = false)}>Cancel</button>
    {:else}
      <button type="button" class="plain" onclick={() => (confirmDisconnect = true)}>Disconnect…</button>
    {/if}
  </SettingRow>
{:else}
  <p class="scope">
    <span class="badge">This graph</span>
    This graph is a folder on this device only.
  </p>
  <p>
    To keep notes on a Nextcloud or other WebDAV server, create a graph there: open the start screen (Ctrl+K,
    "Switch graph") and choose <em>New graph on a server</em>. The same folder can then be opened on other
    devices.
  </p>
{/if}

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 0.8em;
  }
  .last,
  .warn {
    margin: 0.4em 0;
    color: var(--muted);
    font-size: 0.9em;
  }
  .warn {
    color: var(--danger);
  }
  .box {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.8em 1em;
    margin: 0.8em 0;
    display: flex;
    flex-direction: column;
    gap: 0.5em;
    align-items: flex-start;
  }
  .errors {
    margin: 0.4em 0;
    padding-left: 1.2em;
    font-size: 0.85em;
    color: var(--danger);
  }
  code {
    word-break: break-all;
  }
</style>
