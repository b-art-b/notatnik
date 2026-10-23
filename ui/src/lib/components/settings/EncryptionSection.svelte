<script lang="ts">
  // Settings → Sync → Encryption (EN-13, EN-14, EN-18): which files are stored encrypted on the server, and the
  // key. Set up, unlock, lock, the keyring switch, the password and the recovery key. The marks themselves are
  // made from a page (Encrypt…) or, for the whole graph, with Encrypt everything…; here they are listed and can be
  // removed.
  import { backend, type EncryptionPreview } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'
  import { encryption, refreshEncryption } from '../../stores/encryption.svelte'
  import SettingRow from './SettingRow.svelte'

  const st = $derived(encryption.status)
  // `$state` fields are plain variables that the template follows; a form is one of these names or none
  let form = $state<'' | 'setup' | 'unlock' | 'password' | 'recovery' | 'reset' | 'rotate'>('')
  let pw = $state('')
  let pw2 = $state('')
  let old = $state('')
  let recoveryShown = $state<string | null>(null) // right after it was made: shown once
  let saved = $state(false)
  // the password was forgotten and a new vault is being made (EN-22): its recovery key has to be confirmed
  let resetting = $state(false)
  // the vault is being replaced on purpose, with the old key unlocked (EN-26), not because the password was lost
  let rotating = $state(false)
  let understood = $state(false)
  // what a new vault encrypts: everything except a list (recommended), or only what is marked (EN-27)
  let scopeChoice = $state<'all' | 'marked'>('all')

  $effect(() => {
    void refreshEncryption()
  })

  function close() {
    form = ''
    pw = pw2 = old = ''
    understood = false
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
      recoveryShown = (await backend.call('encryption_setup', { password: pw, scope: scopeChoice }))
        .recovery_key
      saved = false
      close()
      await refreshEncryption()
    })
  const confirm = () =>
    run(async () => {
      encryption.status = await backend.call('encryption_confirm', {})
      recoveryShown = null
      toast(
        rotating
          ? 'The key was replaced; the files are encrypted again at the next sync'
          : resetting
            ? 'The password was reset'
            : 'Encryption is on for this graph',
      )
      resetting = false
      rotating = false
    })
  const rotate = () =>
    run(async () => {
      recoveryShown = (await backend.call('encryption_rotate', { password: pw })).recovery_key
      saved = false
      resetting = true
      rotating = true
      close()
      await refreshEncryption()
    })
  const hideNames = (hidden: boolean) =>
    run(async () => {
      encryption.status = await backend.call('encryption_names', { hidden })
    })
  const reset = () =>
    run(async () => {
      recoveryShown = (await backend.call('encryption_setup', { password: pw, reset: true })).recovery_key
      saved = false
      resetting = true
      close()
      await refreshEncryption()
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
  // `encryption_mark` edits the list of the scope: with `all` the exceptions, so `encrypted: true` removes one
  const unmark = (path: string) =>
    run(async () => {
      encryption.status = await backend.call('encryption_mark', { paths: [path], encrypted: false })
    })
  const encryptAgain = (path: string) =>
    run(async () => {
      encryption.status = await backend.call('encryption_mark', { paths: [path], encrypted: true })
    })
  let exception = $state('')
  const addException = () =>
    run(async () => {
      encryption.status = await backend.call('encryption_mark', {
        paths: [exception.trim()],
        encrypted: false,
      })
      exception = ''
    })

  // The scope switch: Encrypt everything… (preview first), and back to only the marked paths
  let everything = $state(false) // the confirmation is open
  let everythingPreview = $state<EncryptionPreview | null>(null)
  const openEverything = () =>
    run(async () => {
      everythingPreview = await backend.call('encryption_scope_preview', { all: st?.scope !== 'all' })
      everything = true
    })
  const switchScope = () =>
    run(async () => {
      const all = st?.scope !== 'all'
      encryption.status = await backend.call('encryption_scope', { all })
      everything = false
      toast(
        all
          ? 'Everything is stored encrypted on the server from the next sync'
          : 'Only the marked paths are stored encrypted from the next sync',
      )
    })
  const copy = (text: string) => void navigator.clipboard?.writeText(text)
</script>

<h3>Encryption</h3>
{#if st}
  <p class="intro">
    Files can be stored <strong>encrypted</strong> on the server: everything except what you list, or only
    what you choose. On this device everything is plain Markdown, so you edit, search and link both kinds the
    same way. Choose from a page (<em>Encrypt…</em> next to <em>History</em>), or everything at once below.
    File names, folders stay hidden too when <em>Hide file and folder names</em> is on (the default for new encryption);
    sizes and times stay visible on the server, the text does not.
  </p>
  {#if st.configured}
    <p class="muted" role="note" data-testid="enc-padlock-note">
      {#if st.scope === 'all'}
        Everything is encrypted, so encrypted pages show no padlock. A page stored as plain text on the server
        shows an open padlock 🔓.
      {:else}
        A closed padlock 🔒 marks a page stored encrypted on the server; pages without one are stored as they
        are.
      {/if}
    </p>
  {/if}
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
      {#if st.configured && !resetting}
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
          >{rotating ? 'Replace the key' : resetting ? 'Replace the password' : 'Turn on encryption'}</button
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
        >Password (8 to 200 characters)
        <input type="password" autocomplete="new-password" bind:value={pw} /></label
      >
      <label>Password again <input type="password" autocomplete="new-password" bind:value={pw2} /></label>
      <fieldset class="scope">
        <legend>What to encrypt on the server</legend>
        <label class="check"
          ><input
            type="radio"
            name="enc-scope"
            value="all"
            bind:group={scopeChoice}
            data-testid="enc-scope-all"
          />
          <span>Everything, except what I list <em>(recommended: a file you forget can never leak)</em></span
          ></label
        >
        <label class="check"
          ><input
            type="radio"
            name="enc-scope"
            value="marked"
            bind:group={scopeChoice}
            data-testid="enc-scope-marked"
          />
          <span>Only what I choose</span></label
        >
      </fieldset>
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
        >New password (8 to 200 characters)
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
  {:else if form === 'rotate'}
    <form
      class="encform"
      data-testid="graph-enc-rotate-form"
      onsubmit={(e) => {
        e.preventDefault()
        if (understood && passwordOk() && st.unlocked) void rotate()
      }}
    >
      <p>
        Makes a new key and a new password, then stores every encrypted file on the server again with it and
        removes the old copies. The text is encrypted on this device and never leaves it as plain text. Use it
        when the old key or password may have been seen by someone. Other devices need the new password.
      </p>
      <p class="warn">
        Earlier versions of the files stay in Nextcloud's file versions and trash, and backups made before
        keep the old key: anyone who has the old key can still read those. Empty the versions and the trash in
        Nextcloud afterwards.
      </p>
      {#if !st.unlocked}<p class="warn">Unlock the key first.</p>{/if}
      <label class="check"
        ><input type="checkbox" bind:checked={understood} data-testid="graph-enc-rotate-understood" /> I understand</label
      >
      <label
        >New password (8 to 200 characters)
        <input type="password" autocomplete="new-password" bind:value={pw} /></label
      >
      <label>New password again <input type="password" autocomplete="new-password" bind:value={pw2} /></label>
      <span class="open">
        <button type="submit" disabled={!understood || !passwordOk() || !st.unlocked}
          >Re-encrypt with a new key</button
        >
        <button type="button" class="plain" onclick={close}>Cancel</button>
      </span>
    </form>
  {:else if form === 'reset'}
    <form
      class="encform"
      data-testid="graph-enc-reset-form"
      onsubmit={(e) => {
        e.preventDefault()
        if (understood && passwordOk()) void reset()
      }}
    >
      <p class="warn">
        Use this only if the password and the recovery key are both lost. The old key is thrown away. Files
        that are stored encrypted on the server and exist as plain files on this device are encrypted again
        with the new key at the next sync; nothing on this device changes. What exists only on the server, or
        only on another device, stays unreadable and is lost. Other devices get the new key file and need the
        new password.
      </p>
      <label class="check"
        ><input type="checkbox" bind:checked={understood} data-testid="graph-enc-reset-understood" /> I understand
        that encrypted data that is not on this device is lost</label
      >
      <label
        >New password (8 to 200 characters)
        <input type="password" autocomplete="new-password" bind:value={pw} /></label
      >
      <label>New password again <input type="password" autocomplete="new-password" bind:value={pw2} /></label>
      <span class="open">
        <button type="submit" disabled={!understood || !passwordOk()}>Reset the password</button>
        <button type="button" class="plain" onclick={close}>Cancel</button>
      </span>
    </form>
  {/if}
  {#if st.configured}
    <p class="more">
      <button type="button" class="plain" onclick={() => (form = 'password')}>Change password…</button>
      <button type="button" class="plain" onclick={() => (form = 'recovery')}>New recovery key…</button>
      <button
        type="button"
        class="plain"
        data-testid="graph-enc-rotate"
        onclick={() => (form = form === 'rotate' ? '' : 'rotate')}>Re-encrypt with a new key…</button
      >
      <button
        type="button"
        class="plain"
        data-testid="graph-enc-forgot"
        onclick={() => (form = form === 'reset' ? '' : 'reset')}>Forgot the password…</button
      >
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
      title="Hide file and folder names"
      description="On: each encrypted file is one object on the server with a name that tells nothing, and its real name and folder are inside, so the server sees no page titles. Off: the server sees the names (with .enc). Switching stores every encrypted file again at the next sync, which needs the key unlocked. The objects are in the folder .notatnik-enc on the server. Its name starts with a dot, so the Nextcloud web app and its mobile apps hide it until you turn on Show hidden files in the Files settings; the sync itself sees it as any other folder. Folders that are empty after the move are removed. The list of encrypted folders in the settings file stays readable, so use Encrypt everything below if you do not want it to name private folders."
      labelFor="graph-enc-names"
    >
      <input
        id="graph-enc-names"
        class="switch"
        type="checkbox"
        role="switch"
        checked={st.names === 'hidden'}
        onchange={(e) => void hideNames((e.currentTarget as HTMLInputElement).checked)}
      />
    </SettingRow>
    <SettingRow
      title="Tasks of encrypted pages in the calendar"
      description="Off (the default): tasks on encrypted pages are not sent to the calendar server, because their text would be readable there. When everything is encrypted, no task is sent unless its page stays plain."
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

  {#if st.configured}
    <SettingRow
      title="Encrypt everything"
      description={st.scope === 'all'
        ? 'Every file is stored encrypted on the server, new ones too, except the ones you list below. Only the settings in the .notatnik folder stay readable.'
        : 'Stores all pages, journals, pictures and templates encrypted on the server, in one step, and every file you add later. You then list what should stay plain. Only the settings in the .notatnik folder stay readable.'}
    >
      {#if st.scope === 'all'}
        <span class="state" data-testid="enc-all-done">Everything is encrypted</span>
        <button
          type="button"
          class="plain"
          data-testid="enc-all-off"
          disabled={!st.unlocked}
          title={st.unlocked ? '' : 'Unlock the key first'}
          onclick={() => (everything ? (everything = false) : openEverything())}
          >Only encrypt what I choose…</button
        >
      {:else}
        <button
          type="button"
          data-testid="enc-all"
          disabled={!st.unlocked}
          title={st.unlocked ? '' : 'Unlock the key first'}
          onclick={() => (everything ? (everything = false) : openEverything())}>Encrypt everything…</button
        >
      {/if}
    </SettingRow>
    {#if everything && everythingPreview}
      <div class="encform" role="group" aria-label="Encrypt everything" data-testid="enc-all-form">
        {#if st.scope === 'all'}
          <p>
            <strong>{everythingPreview.files}</strong> page{everythingPreview.files === 1 ? '' : 's'} will be stored
            as plain text on the server again at the next sync, unless a path you marked covers
            {everythingPreview.files === 1 ? 'it' : 'them'}. On this device nothing changes.
          </p>
          <span class="open">
            <button type="button" onclick={switchScope} data-testid="enc-all-stop"
              >Only encrypt what I choose</button
            >
            <button type="button" class="plain" onclick={() => (everything = false)}>Cancel</button>
          </span>
        {:else}
          <p>
            <strong>{everythingPreview.files}</strong> page{everythingPreview.files === 1 ? '' : 's'} and everything
            else, now and from here on, will be stored encrypted on the server at the next sync. On this device
            nothing changes.
          </p>
          {#if everythingPreview.synced_before}
            <p class="warn" data-testid="enc-all-warn-synced">
              The server already holds this text from earlier syncs. Its file versions and its trash bin keep
              it, whatever is done now: delete them in Nextcloud if that matters.
            </p>
          {/if}
          {#if everythingPreview.calendar_tasks && st.calendar === 'exclude'}
            <p class="muted">
              {everythingPreview.calendar_tasks} task{everythingPreview.calendar_tasks === 1 ? '' : 's'} with a
              date will not be sent to the calendar.
            </p>
          {/if}
          <p class="muted">
            Other apps (the Nextcloud web viewer, its mobile apps) can no longer read the notes. Keep the
            recovery key somewhere safe.
          </p>
          <span class="open">
            <button type="button" onclick={switchScope} data-testid="enc-all-start">Encrypt everything</button
            >
            <button type="button" class="plain" onclick={() => (everything = false)}>Cancel</button>
          </span>
        {/if}
      </div>
    {/if}
  {/if}

  {#if st.configured && !st.unlocked && st.left_alone > 0}
    <p class="warn" role="note" data-testid="enc-left-alone">
      {st.left_alone} encrypted file{st.left_alone === 1 ? ' waits' : 's wait'} for the key: nothing of
      {st.left_alone === 1 ? 'it is' : 'them is'} sent or received, and nothing is deleted. Plain files sync as
      usual.
    </p>
  {/if}
  {#if st.scope === 'all' || st.paths.length}
    <p class="muted" role="note" data-testid="enc-backups-note">
      Backups save these files encrypted with this graph's key too. Restoring one of them, or comparing it,
      needs the key unlocked.
    </p>
  {/if}
  {#if st.reserved.length}
    <p class="warn" role="note" data-testid="enc-reserved">
      A name ending in <code>.enc</code> is reserved for encrypted copies, so these files are not synced:
      {st.reserved.join(', ')}
    </p>
  {/if}

  <div class="paths" data-testid="enc-paths">
    {#if st.scope === 'all'}
      <strong>Plain on the server</strong>
      <p class="muted">
        Everything else is encrypted. A folder ends in <code>/</code>. Page by page you can also use
        <em>Encrypt…</em> on the page.
      </p>
      {#if st.except.length}
        <ul>
          {#each st.except as p (p)}
            <li>
              <code>{p}</code>
              <span class="muted">{p.endsWith('/') ? 'folder and everything below' : 'file'}</span>
              <button
                type="button"
                class="plain"
                disabled={!st.unlocked}
                title={st.unlocked ? '' : 'Unlock first'}
                onclick={() => encryptAgain(p)}>Encrypt again</button
              >
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">Nothing: all files are encrypted.</p>
      {/if}
      <form
        class="open"
        onsubmit={(e) => {
          e.preventDefault()
          if (exception.trim()) void addException()
        }}
      >
        <input
          type="text"
          aria-label="Path to keep plain"
          placeholder="pages/public/"
          bind:value={exception}
          data-testid="enc-except-input"
        />
        <button type="submit" disabled={!st.unlocked || !exception.trim()} data-testid="enc-except-add"
          >Keep plain</button
        >
      </form>
    {:else}
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
  .scope {
    border: 0;
    padding: 0;
    margin: 0.2em 0;
    display: flex;
    flex-direction: column;
    gap: 0.3em;
  }
  .scope legend {
    padding: 0;
    margin-bottom: 0.2em;
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
