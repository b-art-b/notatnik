<script lang="ts">
  // Picks a folder on the server (Nextcloud) by clicking through it, instead of typing its path. It only looks:
  // a folder made here ("New folder") exists on the server when the graph is created in it.
  import { backend, type WebdavFolders } from '../api'
  import { errorMessage } from '../stores/app.svelte'

  let {
    login,
    start,
    onpick,
    onclose,
  }: {
    /** The server, user and password typed in the form. */
    login: { server: string; user: string; password: string }
    /** The folder to open first (the form's folder field). */
    start: string
    onpick: (folder: string) => void
    onclose: () => void
  } = $props()

  // `path` is where we are, as a list of names; `view` is what the server said about it. `start` only says where to
  // begin, so reading it once is what is meant.
  // svelte-ignore state_referenced_locally
  let path = $state<string[]>(
    start
      .split('/')
      .map((s) => s.trim())
      .filter(Boolean),
  )
  let view = $state<WebdavFolders | null>(null)
  let error = $state('')
  let busy = $state(false)
  let newName = $state('')

  const folder = $derived(path.join('/'))

  async function load() {
    busy = true
    error = ''
    try {
      view = await backend.call('webdav_browse', { ...login, folder, attach: false })
    } catch (e) {
      error = errorMessage(e)
      view = null
    } finally {
      busy = false
    }
  }

  // loads whenever `folder` changes (and once at the start)
  $effect(() => {
    void folder
    void load()
  })

  function addFolder() {
    const n = newName.trim().replace(/[/\\]/g, '')
    if (!n || n === '.' || n === '..') return
    path = [...path, n]
    newName = ''
  }
</script>

<div class="browser" role="group" aria-label="Folders on the server" data-testid="webdav-browser">
  <nav class="crumbs" aria-label="Where you are">
    <button
      type="button"
      onclick={() => (path = [])}
      aria-current={path.length === 0 ? 'location' : undefined}>All files</button
    >
    {#each path as name, i (i)}
      <span aria-hidden="true">›</span>
      <button
        type="button"
        onclick={() => (path = path.slice(0, i + 1))}
        aria-current={i === path.length - 1 ? 'location' : undefined}>{name}</button
      >
    {/each}
  </nav>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {:else if busy && !view}
    <p class="muted">Looking…</p>
  {:else if view}
    <ul class="folders" aria-label="Folders">
      {#if path.length}
        <li><button type="button" class="up" onclick={() => (path = path.slice(0, -1))}>↰ Up</button></li>
      {/if}
      {#each view.folders as name (name)}
        <li>
          <button type="button" class="dir" onclick={() => (path = [...path, name])}
            ><span aria-hidden="true">📁</span> {name}</button
          >
        </li>
      {/each}
    </ul>
    {#if !view.exists}
      <p class="muted">This folder does not exist yet. It is created with the graph.</p>
    {:else if view.folders.length === 0}
      <p class="muted">No folders in here.</p>
    {/if}
    {#if view.has_graph}<p class="muted" data-testid="browser-graph">
        This folder holds a Notatnik graph.
      </p>{/if}
  {/if}

  <div class="new">
    <label for="wd-new-folder">New folder here</label>
    <input
      id="wd-new-folder"
      bind:value={newName}
      placeholder="name"
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          addFolder()
        }
      }}
    />
    <button type="button" class="plain" disabled={!newName.trim()} onclick={addFolder}>Add</button>
  </div>

  <div class="buttons">
    <button
      type="button"
      disabled={path.length === 0 || !!error}
      onclick={() => onpick(folder)}
      data-testid="browser-pick">Use {path.length ? `“${path[path.length - 1]}”` : 'a folder'}</button
    >
    <button type="button" class="plain" onclick={onclose}>Cancel</button>
  </div>
</div>

<style>
  .browser {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.6em 0.8em;
    margin: 0.3em 0;
    background: var(--panel);
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.2em;
    font-size: 0.9em;
    margin-bottom: 0.4em;
  }
  .crumbs button {
    border: none;
    background: none;
    color: var(--link);
    cursor: pointer;
    padding: 0.1em 0.3em;
    border-radius: 4px;
    font: inherit;
  }
  .crumbs button[aria-current='location'] {
    color: var(--text);
    font-weight: 600;
  }
  .folders {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 14em;
    overflow: auto;
  }
  .folders button {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    color: inherit;
    padding: 0.3em 0.4em;
    border-radius: 6px;
    cursor: pointer;
    font: inherit;
  }
  .folders button:hover,
  .folders button:focus-visible {
    background: var(--chip);
  }
  .up {
    color: var(--muted);
  }
  .new {
    display: flex;
    align-items: center;
    gap: 0.4em;
    margin-top: 0.5em;
  }
  .new label {
    font-size: 0.85em;
    color: var(--muted);
    white-space: nowrap;
    margin: 0;
  }
  .new input {
    flex: 1;
    min-width: 0;
  }
  .buttons {
    display: flex;
    gap: 0.5em;
    margin-top: 0.6em;
  }
  .muted {
    color: var(--muted);
    font-size: 0.9em;
    margin: 0.3em 0;
  }
  .error {
    color: var(--danger);
    margin: 0.3em 0;
  }
</style>
