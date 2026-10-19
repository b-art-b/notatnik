<script lang="ts">
  // Settings → Sync → Encryption (EN-13, EN-14, EN-18): which files are stored encrypted on the server, and the
  // key. Set up, unlock, lock, the keyring switch, the password and the recovery key. The marks themselves are
  // made from a page (Encrypt…); here they are listed and can be removed.
  import { backend } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'
  import { encryption, refreshEncryption } from '../../stores/encryption.svelte'
  import SettingRow from './SettingRow.svelte'

  const st = $derived(encryption.status)
  // `$state` fields are plain variables that the template follows; a form is one of these names or none
  let form = $state<'' | 'setup' | 'unlock' | 'password' | 'recovery'>('')
  let pw = $state('')
  let pw2 = $state('')
  let old = $state('')
  let recoveryShown = $state<string | null>(null) // right after it was made: shown once
  let saved = $state(false)

  $effect(() => {
    void refreshEncryption()
  })

  function close() {
    form = ''
    pw = pw2 = old = ''
  }
  const passwordOk = () => pw.length >= 8 && pw === pw2
  async function run(work: () => Promise<void>) {
    try {
      await work()
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
  const setup = () =>
    run(async () => {
      recoveryShown = (await backend.call('encryption_setup', { password: pw })).recovery_key
      saved = false
      close()
      await refreshEncryption()
    })
  const confirm = () =>
    run(async () => {
      encryption.status = await backend.call('encryption_confirm', {})
      recoveryShown = null
      toast('Encryption is on for this graph')
    })
  const unlock = () =>
    run(async () => {
      encryption.status = await backend.call('encryption_unlock', { secret: pw })
      close()
    })
  const lock = () =>
    run(async () => {
      encryption.status = await backend.call('encryption_lock', {})
    })
  const changePassword = () =>
    run(async () => {
      encryption.status = await backend.call('encryption_change_password', {
        old_password: old,
        new_password: pw,
      })
      close()
      toast('Password changed')
    })
  const newRecovery = () =>
    run(async () => {
      recoveryShown = (await backend.call('encryption_new_recovery_key', { password: old })).recovery_key
      saved = true // already in use: there is nothing to confirm, only to save
      close()
    })
  const keyring = (on: boolean) =>
    run(async () => {
      encryption.status = await backend.call('encryption_keyring', { on })
    })
  const allowCalendar = (allow: boolean) =>
    run(async () => {
      encryption.status = await backend.call('encryption_calendar', { allow })
    })
  const unmark = (path: string) =>
    run(async () => {
      encryption.status = await backend.call('encryption_mark', { paths: [path], encrypted: false })
    })
  const copy = (text: string) => void navigator.clipboard?.writeText(text)
</script>

<h3>Encryption</h3>
{#if st}
  <p class="intro">
    Some files can be stored <strong>encrypted</strong> on the server while the rest stays as it is. On this
    device everything is plain Markdown, so you edit, search and link both kinds the same way. Choose what to
    encrypt from a page (<em>Encrypt…</em> next to <em>History</em>). File names, folders and sizes stay
    visible on the server; the text does not.
  </p>
  <SettingRow
    title="Key"
    description="The key opens the encrypted files on this device. It stays in memory until you lock it or close the app."
  >
    <span class="state" data-testid="enc-state">
      {#if !st.configured}Not set up{:else if st.unlocked}Unlocked{:else}Locked{/if}
    </span>
    {#if !st.configured}
      <button
        type="button"
        data-testid="graph-enc-setup"
        onclick={() => (form = form === 'setup' ? '' : 'setup')}>Set up…</button
      >
    {:else if st.unlocked}
      <button type="button" class="plain" data-testid="graph-enc-lock" onclick={lock}>Lock</button>
    {:else}
      <button
        type="button"
        data-testid="graph-enc-unlock"
        onclick={() => (form = form === 'unlock' ? '' : 'unlock')}>Unlock…</button
      >
    {/if}
  </SettingRow>

  {#if recoveryShown}
    <div class="recovery" role="group" aria-label="Recovery key" data-testid="graph-recovery-box">
      <strong>Your recovery key</strong>
      <p class="muted">
        It opens the encrypted files if you forget the password. It is shown only now. Write it down or put it
        in a password manager, away from the server. Without the password and this key, nobody can read them.
      </p>
      <code class="key" data-testid="graph-recovery-key">{recoveryShown}</code>
      <span class="open"
        ><button type="button" class="plain" onclick={() => recoveryShown && copy(recoveryShown)}>Copy</button
        ></span
      >
      {#if st.configured}
        <button
          type="button"
          onclick={() => {
            recoveryShown = null
            saved = false
          }}>Done</button
        >
      {:else}
        <label class="check"
          ><input type="checkbox" bind:checked={saved} data-testid="graph-enc-saved" /> I saved the recovery key</label
        >
        <button type="button" disabled={!saved} onclick={confirm} data-testid="graph-enc-confirm"
          >Turn on encryption</button
        >
      {/if}
    </div>
  {/if}

  {#if form === 'setup'}
    <form
      class="encform"
      onsubmit={(e) => {
        e.preventDefault()
        if (passwordOk()) void setup()
      }}
    >
      <label
        >Password (at least 8 characters)
        <input type="password" autocomplete="new-password" bind:value={pw} /></label
      >
      <label>Password again <input type="password" autocomplete="new-password" bind:value={pw2} /></label>
      <span class="open">
        <button type="submit" disabled={!passwordOk()}>Create</button>
        <button type="button" class="plain" onclick={close}>Cancel</button>
      </span>
    </form>
  {:else if form === 'unlock'}
    <form
      class="encform"
      onsubmit={(e) => {
        e.preventDefault()
        void unlock()
      }}
    >
      <label
        >Password or recovery key
        <input type="password" autocomplete="current-password" bind:value={pw} /></label
      >
      <span class="open">
        <button type="submit" disabled={!pw}>Unlock</button>
        <button type="button" class="plain" onclick={close}>Cancel</button>
      </span>
    </form>
  {:else if form === 'password'}
    <form
      class="encform"
      onsubmit={(e) => {
        e.preventDefault()
        if (passwordOk()) void changePassword()
      }}
    >
      <label
        >Current password <input type="password" autocomplete="current-password" bind:value={old} /></label
      >
      <label
        >New password (at least 8 characters)
        <input type="password" autocomplete="new-password" bind:value={pw} /></label
      >
      <label>New password again <input type="password" autocomplete="new-password" bind:value={pw2} /></label>
      <span class="open">
        <button type="submit" disabled={!old || !passwordOk()}>Change password</button>
        <button type="button" class="plain" onclick={close}>Cancel</button>
      </span>
    </form>
  {:else if form === 'recovery'}
    <form
      class="encform"
      onsubmit={(e) => {
        e.preventDefault()
        void newRecovery()
      }}
    >
      <label
        >Current password <input type="password" autocomplete="current-password" bind:value={old} /></label
      >
      <p class="muted">The old recovery key stops working.</p>
      <span class="open">
        <button type="submit" disabled={!old}>Make a new recovery key</button>
        <button type="button" class="plain" onclick={close}>Cancel</button>
      </span>
    </form>
  {/if}
  {#if st.configured}
    <p class="more">
      <button type="button" class="plain" onclick={() => (form = 'password')}>Change password…</button>
      <button type="button" class="plain" onclick={() => (form = 'recovery')}>New recovery key…</button>
    </p>
    <SettingRow
      title="Keep the key in the system keyring"
      description="The app then unlocks itself when the graph opens. Anyone who is logged in to this computer can use the key; the files on the server stay protected from everyone else. Needs an unlocked key."
      labelFor="graph-enc-keyring"
    >
      <input
        id="graph-enc-keyring"
        class="switch"
        type="checkbox"
        role="switch"
        checked={st.keyring}
        disabled={!st.keyring && !st.unlocked}
        onchange={(e) => void keyring((e.currentTarget as HTMLInputElement).checked)}
      />
    </SettingRow>
    <SettingRow
      title="Tasks of encrypted pages in the calendar"
      description="Off (the default): tasks on encrypted pages are not sent to the calendar server, because their text would be readable there."
      labelFor="graph-enc-calendar"
    >
      <input
        id="graph-enc-calendar"
        class="switch"
        type="checkbox"
        role="switch"
        checked={st.calendar === 'allow'}
        onchange={(e) => void allowCalendar((e.currentTarget as HTMLInputElement).checked)}
      />
    </SettingRow>
  {/if}

  {#if st.configured && !st.unlocked && st.left_alone > 0}
    <p class="warn" role="note" data-testid="enc-left-alone">
      {st.left_alone} encrypted file{st.left_alone === 1 ? ' waits' : 's wait'} for the key: nothing of
      {st.left_alone === 1 ? 'it is' : 'them is'} sent or received, and nothing is deleted. Plain files sync as
      usual.
    </p>
  {/if}
  {#if st.backups_plain}
    <p class="warn" role="note" data-testid="enc-backups-plain">
      Your backups are not encrypted, so they hold the encrypted pages as plain text. Turn on encryption for
      backups in Settings → Backup.
    </p>
  {/if}
  {#if st.reserved.length}
    <p class="warn" role="note" data-testid="enc-reserved">
      A name ending in <code>.enc</code> is reserved for encrypted copies, so these files are not synced:
      {st.reserved.join(', ')}
    </p>
  {/if}

  <div class="paths" data-testid="enc-paths">
    <strong>Encrypted on the server</strong>
    {#if st.paths.length}
      <ul>
        {#each st.paths as p (p)}
          <li>
            <code>{p}</code>
            <span class="muted">{p.endsWith('/') ? 'folder and everything below' : 'file'}</span>
            <button
              type="button"
              class="plain"
              disabled={!st.unlocked}
              title={st.unlocked ? '' : 'Unlock first: the file is stored again as plain text'}
              onclick={() => unmark(p)}>Stop encrypting</button
            >
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">Nothing yet.</p>
    {/if}
  </div>
{/if}

<style>
  h3 {
    margin: 1.6em 0 0.4em;
  }
  .intro {
    color: var(--muted);
    font-size: 0.9em;
    margin: 0 0 0.6em;
  }
  .state {
    margin-right: 0.8em;
    font-weight: 600;
  }
  .recovery,
  .encform,
  .paths {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.8em 1em;
    margin: 0.8em 0;
    display: flex;
    flex-direction: column;
    gap: 0.5em;
    align-items: flex-start;
  }
  .key {
    font-size: 1.05em;
    letter-spacing: 0.04em;
    word-break: break-all;
  }
  .encform label {
    display: flex;
    flex-direction: column;
    gap: 0.2em;
  }
  .check {
    flex-direction: row;
    gap: 0.5em;
    align-items: center;
  }
  .more {
    margin: 0.2em 0;
  }
  .muted {
    color: var(--muted);
    font-size: 0.85em;
    margin: 0;
  }
  .warn {
    color: var(--danger);
    font-size: 0.9em;
    margin: 0.4em 0;
  }
  .paths ul {
    margin: 0;
    padding-left: 1.2em;
  }
  .paths li {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6em;
    align-items: center;
  }
</style>
