<script lang="ts">
  // "New graph on a server" on the start screen: the address, login and folder of a WebDAV share (Nextcloud). The
  // folder is looked at first without changing anything; an empty or new folder becomes a new graph, a folder that
  // already holds a graph can be downloaded to this device.
  import { backend, CommandError, type WebdavTest } from '../api'
  import { adoptGraph, errorMessage } from '../stores/app.svelte'
  import WebdavFolderBrowser from './WebdavFolderBrowser.svelte'

  let server = $state('')
  let user = $state('')
  let password = $state('')
  let folder = $state('Notes/notatnik')
  let seen = $state<WebdavTest | null>(null)
  let error = $state('')
  let busy = $state(false)
  let browsing = $state(false)

  const args = (attach: boolean) => ({ server, user, password, folder, attach })
  const filled = $derived(!!server.trim() && !!user.trim() && !!password && !!folder.trim())

  /** Looks at the folder; the result decides what the main button does. */
  async function check() {
    busy = true
    error = ''
    seen = null
    try {
      seen = await backend.call('webdav_test', args(false))
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  async function create(attach: boolean) {
    busy = true
    error = ''
    try {
      await adoptGraph(await backend.call('create_remote_graph', args(attach)))
    } catch (e) {
      error = errorMessage(e)
      // the folder has a graph after all: show the choice
      if (e instanceof CommandError && e.code === 'has_graph') await check()
    } finally {
      busy = false
    }
  }
</script>

<form
  class="webdav"
  data-testid="webdav-form"
  onsubmit={(e) => {
    e.preventDefault()
    void (seen ? create(seen.has_graph) : check())
  }}
>
  <label for="wd-server">Server</label>
  <input id="wd-server" bind:value={server} placeholder="https://cloud.example.org" autocomplete="url" />
  <label for="wd-user">User name</label>
  <input id="wd-user" bind:value={user} autocomplete="username" />
  <label for="wd-pass">Password (on Nextcloud: an app password from Settings → Security)</label>
  <input id="wd-pass" type="password" bind:value={password} autocomplete="current-password" />
  <label for="wd-folder">Folder on the server</label>
  <div class="row">
    <input id="wd-folder" bind:value={folder} oninput={() => (seen = null)} />
    <button
      type="button"
      class="plain"
      data-testid="webdav-browse"
      disabled={!server.trim() || !user.trim() || !password || busy}
      title={!server.trim() || !user.trim() || !password
        ? 'Fill in the server, user name and password first'
        : ''}
      onclick={() => (browsing = !browsing)}>Browse…</button
    >
  </div>
  {#if browsing}
    <WebdavFolderBrowser
      login={{ server, user, password }}
      start={folder}
      onpick={(f) => {
        folder = f
        seen = null
        browsing = false
        void check()
      }}
      onclose={() => (browsing = false)}
    />
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if seen}
    <p class="seen" data-testid="webdav-seen">
      {#if !seen.exists}
        The folder does not exist yet; it will be created.
      {:else if seen.items === 0}
        The folder is empty.
      {:else if seen.has_graph}
        The folder already holds a Notatnik graph. It will be downloaded to this device.
      {:else}
        The folder holds {seen.items} other item(s). Choose an empty or a new folder.
      {/if}
    </p>
  {/if}
  <div class="buttons">
    <button
      type="submit"
      disabled={!filled || busy || (!!seen && seen.exists && seen.items > 0 && !seen.has_graph)}
    >
      {#if busy}Working…{:else if !seen}Check the folder{:else if seen.has_graph}Download and open{:else}Create
        the graph{/if}
    </button>
  </div>
</form>

<style>
  .webdav {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.8em 1em;
    margin-bottom: 0.5em;
  }
  label {
    font-size: 0.85em;
    color: var(--muted);
    margin-top: 0.4em;
  }
  input {
    padding: 0.5em;
    box-sizing: border-box;
    width: 100%;
  }
  .row {
    display: flex;
    gap: 0.4em;
  }
  .row input {
    flex: 1;
    min-width: 0;
  }
  .error {
    color: var(--danger);
    margin: 0.4em 0 0;
  }
  .seen {
    margin: 0.4em 0 0;
  }
  .buttons {
    margin-top: 0.6em;
  }
</style>
