<script lang="ts">
  // Encrypt a page on the server, or stop doing so (EN-16, EN-17, EN-18). It says what marking means before it
  // is done: older plain copies on the server, pictures that stay plain, tasks that leave the calendar.
  import { backend, type EncryptionPreview } from '../api'
  import { errorMessage, toast } from '../stores/app.svelte'
  import { coveringPaths, encryption, refreshEncryption } from '../stores/encryption.svelte'

  // `path`: the file of the page; `hasSubpages`: pages live below it (a folder next to the file)
  let {
    page,
    path,
    hasSubpages,
    onclose,
  }: { page: string; path: string; hasSubpages: boolean; onclose: () => void } = $props()

  const st = $derived(encryption.status)
  let below = $state(false)
  let paths = $state<string[]>([])
  let preview = $state<EncryptionPreview | null>(null)
  let busy = $state(false)
  let error = $state('')

  const covering = $derived(st ? coveringPaths(st.paths, path) : [])
  // the rules of this page that can be removed here; a folder above it can only be removed in Settings
  const own = $derived(st ? st.paths.filter((p) => paths.includes(p)) : [])
  const outer = $derived(covering.filter((p) => !paths.includes(p)))

  $effect(() => {
    void refreshEncryption()
  })
  // `below` decides which paths are meant; read the page's paths whenever it changes
  $effect(() => {
    const subpages = below
    backend
      .call('encryption_page_paths', { page, subpages })
      .then((r) => (paths = r.items))
      .catch((e) => (error = errorMessage(e)))
  })
  $effect(() => {
    const p = paths
    if (!p.length || covering.length) {
      preview = null
      return
    }
    backend
      .call('encryption_preview', { paths: p, encrypted: true })
      .then((r) => (preview = r))
      .catch((e) => (error = errorMessage(e)))
  })

  async function mark(encrypted: boolean) {
    busy = true
    error = ''
    try {
      encryption.status = await backend.call('encryption_mark', {
        paths: encrypted ? paths : own,
        encrypted,
      })
      toast(
        encrypted
          ? 'The page is stored encrypted on the server'
          : 'The page is stored as plain text on the server again',
      )
      onclose()
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }
</script>

<div class="dialog" role="dialog" aria-label="Encrypt page" aria-modal="true" data-testid="encrypt-dialog">
  {#if !st}
    <p class="muted">…</p>
  {:else if !st.has_server}
    <p>
      This graph has no server folder. Encryption applies to the copy on a server (Settings → Sync); on this
      device the page is a plain file.
    </p>
  {:else if !st.configured}
    <p>
      Encryption is not set up for this graph yet. <a href="#/settings" data-testid="encrypt-goto-settings"
        >Set it up in Settings → Sync</a
      >, then come back.
    </p>
  {:else if outer.length && !own.length}
    <p data-testid="encrypt-covered">
      This page is stored encrypted because of <code>{outer.join(', ')}</code>. Change that in Settings →
      Sync.
    </p>
  {:else if own.length}
    <p data-testid="encrypt-already">
      This page is stored <strong>encrypted</strong> on the server{outer.length
        ? ` (also because of ${outer.join(', ')})`
        : ''}.
    </p>
    {#if !st.unlocked}
      <p class="warn">Unlock the key in Settings → Sync first: the file is stored again as plain text.</p>
    {/if}
    <button
      type="button"
      disabled={busy || !st.unlocked}
      onclick={() => mark(false)}
      data-testid="encrypt-stop">Stop encrypting</button
    >
  {:else if !st.unlocked}
    <p>
      The key is locked. <a href="#/settings" data-testid="encrypt-goto-settings"
        >Unlock it in Settings → Sync</a
      >, then come back.
    </p>
  {:else}
    <p>Store <strong>{page}</strong> encrypted on the server. On this device it stays a plain page.</p>
    {#if hasSubpages}
      <label class="check"
        ><input type="checkbox" bind:checked={below} data-testid="encrypt-below" /> Also the pages below it</label
      >
    {/if}
    {#if preview}
      {#if preview.synced_before}
        <p class="warn" data-testid="encrypt-warn-synced">
          The server already holds this text from earlier syncs. Its file versions and its trash bin keep it,
          whatever is done now: delete them in Nextcloud if that matters.
        </p>
      {/if}
      {#if preview.images.length}
        <p class="warn" data-testid="encrypt-warn-images">
          {preview.images.length} picture{preview.images.length === 1 ? '' : 's'} used here stay{preview
            .images.length === 1
            ? 's'
            : ''} plain on the server: {preview.images.join(', ')}
        </p>
      {/if}
      {#if preview.calendar_tasks && st.calendar === 'exclude'}
        <p class="note" data-testid="encrypt-warn-tasks">
          {preview.calendar_tasks} task{preview.calendar_tasks === 1 ? '' : 's'} on {preview.files === 1
            ? 'this page'
            : 'these pages'} will not be sent to the calendar.
        </p>
      {/if}
      {#if st.backups_plain || (!st.paths.length && !preview.synced_before)}
        <p class="note" data-testid="encrypt-warn-backups">
          Backups are made from the plain files here: turn on encrypted backups (Settings → Backup) so that
          this page is not plain there.
        </p>
      {/if}
    {/if}
    <button
      type="button"
      disabled={busy || !paths.length}
      onclick={() => mark(true)}
      data-testid="encrypt-start">Encrypt</button
    >
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <button type="button" class="plain" onclick={onclose}>Close</button>
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
  .note,
  .muted {
    color: var(--muted);
  }
  .error {
    color: var(--danger);
  }
  .check {
    display: flex;
    gap: 0.5em;
    align-items: center;
    margin: 0.4em 0;
  }
</style>
