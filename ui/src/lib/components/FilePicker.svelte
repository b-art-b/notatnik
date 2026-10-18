<script lang="ts">
  // `/file`: links a file into the block. Choose where it is: a server from the list (or add one), this
  // computer, or an address you type. A server is browsed folder by folder; a link is inserted when you pick a
  // file. Nothing is downloaded: a link to a Nextcloud file is its `/f/<number>` address.
  // It takes the focus; the parent decides where the focus goes after `oninsert` / `oncancel`.
  import { onMount } from 'svelte'
  import { backend, type FileBrowse, type FileSourceView, type FolderItem } from '../api'
  import { errorMessage } from '../stores/app.svelte'
  import { toBase64 } from '../images'
  import { fileLink, fileUrl, humanSize, isLinkable, lastName, nameForAddress } from '../filelinks'
  import FileSourceForm from './FileSourceForm.svelte'

  let {
    pageName,
    oninsert,
    oncancel,
  }: {
    /** The page the link goes into (copied files are linked relative to it). */
    pageName: string
    /** The finished Markdown link. */
    oninsert: (markdown: string) => void
    oncancel: () => void
  } = $props()

  type Step = 'start' | 'add' | 'browse' | 'computer' | 'address'
  let step = $state<Step>('start')
  let sources = $state<FileSourceView[] | null>(null)
  let error = $state('')
  let busy = $state(false)
  let root: HTMLDivElement

  // browsing
  let source = $state<FileSourceView | null>(null)
  // the folders entered so far; each keeps its link, so the folder you are in can be linked too
  let trail = $state<FolderItem[]>([])
  const path = $derived(trail.map((f) => f.name))
  let view = $state<FileBrowse | null>(null)
  let filter = $state('')

  // this computer
  let localPath = $state('')

  // an address
  let address = $state('')
  let label = $state('')

  const LAST = 'notatnik-file-source'
  const byPathHint =
    'This server has no folder numbers: the link is the address of the folder and breaks if it is renamed or moved'
  /** The folder being shown, as a link target (the top of the source when nothing was entered). */
  const here = $derived(
    trail.length > 0
      ? trail[trail.length - 1]
      : (view?.root ?? { name: 'All files', link: '', by_path: false }),
  )
  const folder = $derived(path.join('/'))
  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase()
    const keep = (n: string) => !q || n.toLowerCase().includes(q)
    return {
      folders: (view?.folders ?? []).filter((f) => keep(f.name)),
      files: (view?.files ?? []).filter((f) => keep(f.name)),
    }
  })

  onMount(async () => {
    try {
      sources = await backend.call('file_sources', {})
    } catch (e) {
      error = errorMessage(e)
      sources = []
    }
    focusFirst()
  })

  /** Moves the focus to the first control of the current step. */
  function focusFirst() {
    queueMicrotask(() =>
      root?.querySelector<HTMLElement>('input:not([type=hidden]), button:not([disabled])')?.focus(),
    )
  }

  function go(next: Step) {
    step = next
    error = ''
    focusFirst()
  }

  function lastUsed(list: FileSourceView[]): FileSourceView[] {
    let id = ''
    try {
      id = localStorage.getItem(LAST) ?? ''
    } catch {
      // storage can be blocked; the order stays as it is
    }
    return [...list.filter((s) => s.id === id), ...list.filter((s) => s.id !== id)]
  }

  async function open(s: FileSourceView) {
    source = s
    trail = []
    filter = ''
    view = null
    try {
      localStorage.setItem(LAST, s.id)
    } catch {
      // not remembered
    }
    go('browse')
    await load()
  }

  async function load() {
    if (!source) return
    busy = true
    error = ''
    try {
      view = await backend.call('file_browse', { source: source.id, path: folder })
    } catch (e) {
      error = errorMessage(e)
      view = null
    } finally {
      busy = false
    }
  }

  async function enter(t: FolderItem[]) {
    trail = t
    filter = ''
    await load()
    focusFirst()
  }

  async function computer() {
    go('computer')
    if (!backend.pickFile) return // a browser: the file input below
    try {
      const p = await backend.pickFile()
      if (p) localPath = p
      else go('start')
    } catch (e) {
      error = errorMessage(e)
    }
  }

  /** Desktop: the system's folder chooser; the link opens the folder in the file manager. */
  async function linkFolder() {
    if (!backend.pickFolder) return
    try {
      const p = await backend.pickFolder()
      if (p) oninsert(fileLink(lastName(p) || p, fileUrl(p)))
    } catch (e) {
      error = errorMessage(e)
    }
  }

  async function copyIn(args: { path?: string; file?: File }) {
    busy = true
    error = ''
    try {
      const saved = await backend.call('attach_file', {
        page: pageName,
        path: args.path ?? null,
        data: args.file ? await toBase64(args.file) : null,
        name: args.file?.name ?? null,
      })
      oninsert(saved.markdown)
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  function key(e: KeyboardEvent) {
    e.stopPropagation() // keep Enter, Escape and the arrows away from the editor and the global handler
    if (e.key !== 'Escape') return
    e.preventDefault()
    if (step === 'start') oncancel()
    else if (step === 'browse' && path.length) void enter(trail.slice(0, -1))
    else go('start')
  }
</script>

<!-- a click inside must not reach the block's own click handler (it would re-open the editor) -->
<div
  class="file-picker"
  role="dialog"
  aria-label="Link a file or folder"
  tabindex="-1"
  data-testid="file-picker"
  bind:this={root}
  onkeydown={key}
  onclick={(e) => e.stopPropagation()}
>
  {#if step === 'start'}
    <h3>Link a file or folder from…</h3>
    {#if sources === null}
      <p class="muted">Looking…</p>
    {:else}
      <ul class="choices" aria-label="Where the file is">
        {#each lastUsed(sources) as s (s.id)}
          <li>
            <button type="button" data-testid="file-source" onclick={() => open(s)}>
              <span class="ico" aria-hidden="true">☁</span>
              <span class="what"><strong>{s.name}</strong><small>{s.user} · {s.server}</small></span>
            </button>
          </li>
        {/each}
        <li>
          <button type="button" data-testid="file-add-server" onclick={() => go('add')}>
            <span class="ico" aria-hidden="true">＋</span>
            <span class="what"
              ><strong>Add a server…</strong><small>Nextcloud or another WebDAV server</small></span
            >
          </button>
        </li>
        <li>
          <button type="button" data-testid="file-computer" onclick={computer}>
            <span class="ico" aria-hidden="true">🖥</span>
            <span class="what"
              ><strong>This computer…</strong><small
                >Link to a file or folder here, or copy a file into the graph</small
              ></span
            >
          </button>
        </li>
        <li>
          <button type="button" data-testid="file-address" onclick={() => go('address')}>
            <span class="ico" aria-hidden="true">🔗</span>
            <span class="what"><strong>Type an address…</strong><small>Any web address or e-mail</small></span
            >
          </button>
        </li>
      </ul>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  {:else if step === 'add'}
    <h3>Add a server</h3>
    <FileSourceForm
      onsaved={(s) => {
        sources = [...(sources ?? []), s]
        void open(s)
      }}
      oncancel={() => go('start')}
    />
  {:else if step === 'browse' && source}
    <div class="top">
      <button type="button" class="plain" onclick={() => go('start')}>← Servers</button>
      <strong>{source.name}</strong>
    </div>
    <nav class="crumbs" aria-label="Where you are">
      <button
        type="button"
        onclick={() => enter([])}
        aria-current={path.length === 0 ? 'location' : undefined}>All files</button
      >
      {#each path as name, i (i)}
        <span aria-hidden="true">›</span>
        <button
          type="button"
          onclick={() => enter(trail.slice(0, i + 1))}
          aria-current={i === path.length - 1 ? 'location' : undefined}>{name}</button
        >
      {/each}
    </nav>
    {#if view?.exists}
      <button
        type="button"
        class="plain this-folder"
        data-testid="folder-link-here"
        title={here.by_path ? byPathHint : 'Link to this folder'}
        onclick={() => oninsert(fileLink(here.name, here.link))}>🔗 Link this folder: {here.name}</button
      >
    {/if}
    <input
      class="filter"
      bind:value={filter}
      placeholder="Filter by name"
      aria-label="Filter by name"
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          // one file left: that one
          if (shown.files.length === 1 && shown.folders.length === 0) {
            const f = shown.files[0]
            oninsert(fileLink(f.name, f.link))
          }
        }
      }}
    />
    {#if error}
      <p class="error" role="alert">{error}</p>
    {:else if busy && !view}
      <p class="muted">Looking…</p>
    {:else if view}
      <ul class="entries" aria-label="Folders and files">
        {#if path.length}
          <li>
            <button type="button" class="up" onclick={() => enter(trail.slice(0, -1))}>↰ Up</button>
          </li>
        {/if}
        {#each shown.folders as f (f.name)}
          <li class="folder-row">
            <button type="button" class="dir" onclick={() => enter([...trail, f])}
              ><span aria-hidden="true">📁</span> {f.name}</button
            >
            <button
              type="button"
              class="link-folder"
              data-testid="folder-link"
              aria-label={`Link the folder ${f.name}`}
              title={f.by_path ? byPathHint : 'Link to this folder'}
              onclick={() => oninsert(fileLink(f.name, f.link))}>Link</button
            >
          </li>
        {/each}
        {#each shown.files as f (f.name)}
          <li>
            <button
              type="button"
              class="file"
              data-testid="file-entry"
              title={f.by_path
                ? 'This server has no file numbers: the link is the address of the file and breaks if the file is renamed or moved'
                : 'Link to this file'}
              onclick={() => oninsert(fileLink(f.name, f.link))}
              ><span aria-hidden="true">📄</span> <span class="n">{f.name}</span>
              <small>{humanSize(f.size)}</small></button
            >
          </li>
        {/each}
      </ul>
      {#if !view.exists}
        <p class="muted">This folder does not exist.</p>
      {:else if shown.folders.length === 0 && shown.files.length === 0}
        <p class="muted">{filter.trim() ? 'Nothing matches.' : 'This folder is empty.'}</p>
      {/if}
      {#if view.files.some((f) => f.by_path)}
        <p class="muted">
          This server gives no file numbers, so the link is the file's address. It breaks if the file is
          renamed or moved.
        </p>
      {/if}
    {/if}
  {:else if step === 'computer'}
    <div class="top">
      <button type="button" class="plain" onclick={() => go('start')}>← Back</button>
      <strong>This computer</strong>
    </div>
    {#if backend.pickFile}
      {#if localPath}
        <p class="picked" data-testid="file-local-path"><code>{localPath}</code></p>
        <div class="buttons">
          <button
            type="button"
            data-testid="file-link-in-place"
            onclick={() => oninsert(fileLink(lastName(localPath), fileUrl(localPath)))}
            >Link to the file where it is</button
          >
          <button type="button" class="plain" disabled={busy} onclick={() => copyIn({ path: localPath })}
            >Copy it into the graph</button
          >
        </div>
        <p class="muted">
          A link to the file works only on this computer. A copy is stored in the graph (and synced), so the
          link works everywhere; up to 25 MB.
        </p>
      {:else}
        <p class="muted">Waiting for the file chooser…</p>
      {/if}
      <div class="buttons">
        <button type="button" class="plain" data-testid="file-local-folder" onclick={linkFolder}
          >Link a folder instead…</button
        >
      </div>
    {:else}
      <label class="muted" for="file-upload">
        Choose a file to copy into the graph (up to 25 MB). A link to a file in place needs the desktop app.
      </label>
      <input
        id="file-upload"
        type="file"
        data-testid="file-upload"
        disabled={busy}
        onchange={(e) => {
          const f = e.currentTarget.files?.[0]
          if (f) void copyIn({ file: f })
        }}
      />
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  {:else if step === 'address'}
    <div class="top">
      <button type="button" class="plain" onclick={() => go('start')}>← Back</button>
      <strong>An address</strong>
    </div>
    <form
      onsubmit={(e) => {
        e.preventDefault()
        e.stopPropagation()
        const a = address.trim()
        if (!isLinkable(a)) {
          error = 'The address must start with https://, http://, mailto: or file:///'
          return
        }
        oninsert(fileLink(label.trim() || nameForAddress(a), a))
      }}
    >
      <label for="file-address-url">Address</label>
      <input id="file-address-url" bind:value={address} placeholder="https://…" autocomplete="off" />
      <label for="file-address-name">Link text (optional)</label>
      <input
        id="file-address-name"
        bind:value={label}
        placeholder={address.trim() ? nameForAddress(address) : ''}
      />
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="buttons">
        <button type="submit" disabled={!address.trim()}>Insert</button>
      </div>
    </form>
  {/if}
</div>

<style>
  .file-picker {
    width: min(26em, calc(100vw - 2em));
    max-height: min(30em, 70vh);
    overflow: auto;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.25);
    padding: 0.7em 0.8em;
    font-size: 0.95rem;
  }
  h3 {
    margin: 0 0 0.5em;
    font-size: 1em;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .choices button,
  .entries button {
    display: flex;
    align-items: center;
    gap: 0.6em;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    color: inherit;
    padding: 0.4em 0.5em;
    border-radius: 6px;
    cursor: pointer;
    font: inherit;
  }
  .choices button:hover,
  .choices button:focus-visible,
  .entries button:hover,
  .entries button:focus-visible {
    background: var(--chip);
  }
  .ico {
    width: 1.6em;
    text-align: center;
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .what small,
  .file small,
  .muted {
    color: var(--muted);
    font-size: 0.85em;
  }
  .what small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 0.6em;
    margin-bottom: 0.3em;
  }
  .folder-row {
    display: flex;
    gap: 0.3em;
  }
  .folder-row .dir {
    flex: 1;
  }
  .folder-row .link-folder {
    flex: none;
    width: auto;
  }
  .this-folder {
    margin: 0.3em 0;
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
  .filter,
  form input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.4em 0.5em;
  }
  .filter {
    margin-bottom: 0.4em;
  }
  .file .n {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .up {
    color: var(--muted);
  }
  .entries {
    max-height: 16em;
    overflow: auto;
  }
  .picked {
    margin: 0.4em 0;
    word-break: break-all;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5em;
    margin: 0.5em 0;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
  }
  label {
    font-size: 0.85em;
    color: var(--muted);
  }
  .error {
    color: var(--danger);
    margin: 0.4em 0 0;
  }
  .muted {
    margin: 0.4em 0 0;
  }
</style>
