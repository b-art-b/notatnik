<script lang="ts">
  // Adds a server to link files from, or changes one. Saving logs in once, so a wrong address or password is
  // told here. Used in Settings → Linked files and inside the file picker.
  import { backend, type FileSourceView } from '../api'
  import { errorMessage } from '../stores/app.svelte'

  let {
    source,
    onsaved,
    oncancel,
  }: {
    /** The server to change; absent for a new one. */
    source?: FileSourceView
    onsaved: (s: FileSourceView) => void
    oncancel: () => void
  } = $props()

  // the form starts from the server given; later changes to the prop are not meant to overwrite what is typed
  /* svelte-ignore state_referenced_locally */
  let name = $state(source?.name ?? '')
  /* svelte-ignore state_referenced_locally */
  let server = $state(source?.server ?? '')
  /* svelte-ignore state_referenced_locally */
  let user = $state(source?.user ?? '')
  let password = $state('')
  let error = $state('')
  let busy = $state(false)

  const ready = $derived(!!name.trim() && !!server.trim() && !!user.trim() && (!!password || !!source))

  async function save() {
    busy = true
    error = ''
    try {
      onsaved(
        await backend.call('file_source_save', { id: source?.id ?? null, name, server, user, password }),
      )
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }
</script>

<form
  class="source-form"
  data-testid="file-source-form"
  onsubmit={(e) => {
    e.preventDefault()
    e.stopPropagation()
    if (ready && !busy) void save()
  }}
>
  <label for="fs-name">Name</label>
  <input id="fs-name" bind:value={name} placeholder="Nextcloud" />
  <label for="fs-server">Server</label>
  <input id="fs-server" bind:value={server} placeholder="https://cloud.example.org" autocomplete="url" />
  <label for="fs-user">User name</label>
  <input id="fs-user" bind:value={user} autocomplete="username" />
  <label for="fs-pass"
    >Password (on Nextcloud: an app password from Settings → Security){source
      ? '. Leave empty to keep the stored one'
      : ''}</label
  >
  <input id="fs-pass" type="password" bind:value={password} autocomplete="current-password" />
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="buttons">
    <button type="submit" disabled={!ready || busy}>{busy ? 'Logging in…' : 'Save'}</button>
    <button type="button" class="plain" onclick={oncancel}>Cancel</button>
  </div>
</form>

<style>
  .source-form {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
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
  .error {
    color: var(--danger);
    margin: 0.4em 0 0;
  }
  .buttons {
    display: flex;
    gap: 0.5em;
    margin-top: 0.6em;
  }
</style>
