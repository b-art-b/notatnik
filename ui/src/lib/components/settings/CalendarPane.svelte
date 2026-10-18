<script lang="ts">
  // Settings → Calendar: which login and which Nextcloud calendars the graph uses. The login is the one of the
  // graph's sync unless you give it one of its own. Nothing is sent to the server from here except a login
  // check and the list of calendars; the sync itself comes later.
  import { onMount } from 'svelte'
  import { backend, type CalendarList, type CalendarStatus } from '../../api'
  import { errorMessage, toast } from '../../stores/app.svelte'
  import { calendarSyncNow, describeCalendarSync } from '../../calendar'

  let status = $state<CalendarStatus | null>(null)
  let list = $state<CalendarList | null>(null)
  let busy = $state(false)
  let error = $state('')
  let syncing = $state(false)
  let lastSync = $state('')

  // the form for a login of its own
  let own = $state(false)
  let server = $state('')
  let user = $state('')
  let password = $state('')

  // the choices; `$state` copies of what is saved, so typing does not touch the saved status
  let enabled = $state(false)
  let tasks = $state<string | null>(null)
  let read = $state<string[]>([])

  async function load() {
    try {
      status = await backend.call('calendar_status', {})
      enabled = status.enabled
      tasks = status.tasks
      read = [...status.read]
      own = !status.account
    } catch (e) {
      error = errorMessage(e)
    }
  }
  onMount(load)

  async function connect() {
    busy = true
    error = ''
    try {
      list = await backend.call('calendar_connect', {
        server: own ? server : null,
        user: own ? user : null,
        password: own ? password : null,
      })
      password = ''
      status = await backend.call('calendar_status', {})
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  async function save() {
    busy = true
    error = ''
    try {
      status = await backend.call('calendar_save', { enabled, read, tasks })
      toast('Calendar settings saved')
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  async function syncTasks() {
    syncing = true
    const r = await calendarSyncNow()
    if (r) lastSync = describeCalendarSync(r)
    syncing = false
  }

  async function forget() {
    try {
      status = await backend.call('calendar_forget', {})
      list = null
      enabled = false
      toast('The calendar login was removed')
    } catch (e) {
      error = errorMessage(e)
    }
  }

  function toggleRead(href: string, on: boolean) {
    read = on ? [...read.filter((h) => h !== href), href] : read.filter((h) => h !== href)
  }
</script>

<h2>Calendar</h2>
<p class="scope">
  <span class="badge">This device</span>
  Dated tasks go to a Nextcloud calendar as tasks (VTODO), and changes made on the phone (DAVx⁵) or in Nextcloud
  Tasks come back into your notes. Events of your calendars show in the journal.
</p>

{#if status}
  <section aria-label="Login">
    <h3>Login</h3>
    {#if status.account && !status.account.own}
      <p>
        Same login as the sync: <strong>{status.account.user}</strong> on
        <code>{status.account.server}</code>.
      </p>
      <button type="button" class="plain" onclick={() => (own = !own)}>
        {own ? 'Use the sync login' : 'Use another login…'}
      </button>
    {:else if status.account}
      <p>
        Own login: <strong>{status.account.user}</strong> on <code>{status.account.server}</code>.
        <button type="button" class="plain" onclick={forget}>Remove…</button>
      </p>
    {:else}
      <p class="muted">This graph is not synced with a server: enter the Nextcloud address and your login.</p>
    {/if}

    {#if own}
      <form
        class="login"
        data-testid="calendar-login"
        onsubmit={(e) => {
          e.preventDefault()
          if (!busy) void connect()
        }}
      >
        <label for="cal-server">Server</label>
        <input
          id="cal-server"
          bind:value={server}
          placeholder="https://cloud.example.org"
          autocomplete="url"
        />
        <label for="cal-user">User name</label>
        <input id="cal-user" bind:value={user} autocomplete="username" />
        <label for="cal-pass">Password (an app password from Nextcloud Settings → Security)</label>
        <input id="cal-pass" type="password" bind:value={password} autocomplete="current-password" />
        <div class="buttons">
          <button type="submit" disabled={busy || !server.trim() || !user.trim() || !password}
            >{busy ? 'Connecting…' : 'Connect'}</button
          >
        </div>
      </form>
    {:else}
      <div class="buttons">
        <button
          type="button"
          data-testid="calendar-connect"
          disabled={busy || !status.account}
          onclick={connect}
          >{busy ? 'Connecting…' : list ? 'Reload calendars' : 'Connect and list calendars'}</button
        >
      </div>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </section>

  {#if list}
    <section aria-label="Calendars">
      <h3>Calendars</h3>
      <p class="muted">Connected as {list.account.user}: {list.calendars.length} calendars found.</p>
      <table class="cals">
        <thead>
          <tr><th>Calendar</th><th>Receives tasks</th><th>Show events</th></tr>
        </thead>
        <tbody>
          {#each list.calendars as c (c.href)}
            <tr>
              <td>{c.name}</td>
              <td>
                <input
                  type="radio"
                  name="cal-tasks"
                  aria-label={`Tasks go to ${c.name}`}
                  disabled={!c.tasks}
                  checked={tasks === c.href}
                  onchange={() => (tasks = c.href)}
                />
              </td>
              <td>
                <input
                  type="checkbox"
                  aria-label={`Show events of ${c.name}`}
                  disabled={!c.events}
                  checked={read.includes(c.href)}
                  onchange={(e) => toggleRead(c.href, e.currentTarget.checked)}
                />
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <label class="on">
        <input type="checkbox" bind:checked={enabled} /> Calendar sync is on
      </label>
      <div class="buttons">
        <button type="button" data-testid="calendar-save" disabled={busy} onclick={save}>Save</button>
      </div>
    </section>
  {:else if status.enabled}
    <p class="muted">Calendar sync is on. Connect to change the calendars.</p>
  {/if}

  {#if status.enabled && status.tasks}
    <section aria-label="Tasks">
      <h3>Tasks</h3>
      <p class="muted">
        Tasks with a <code>scheduled::</code> or <code>deadline::</code> date go to the calendar; a task gets
        a block ID (<code>^abc123</code>) on its line the first time. Add <code>cal:: no</code> to keep one out.
      </p>
      <div class="buttons">
        <button type="button" data-testid="calendar-sync" disabled={syncing} onclick={syncTasks}
          >{syncing ? 'Sending…' : 'Send tasks to the calendar now'}</button
        >
      </div>
      {#if lastSync}<p class="muted" data-testid="calendar-sync-result">{lastSync}</p>{/if}
    </section>
  {/if}
{/if}

<style>
  section {
    margin: 0.8em 0;
  }
  h3 {
    font-size: 1em;
    margin: 0.6em 0 0.3em;
  }
  .login {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
    max-width: 28em;
    margin-top: 0.6em;
  }
  .login label {
    font-size: 0.85em;
    color: var(--muted);
    margin-top: 0.4em;
  }
  .login input {
    padding: 0.5em;
    box-sizing: border-box;
    width: 100%;
  }
  .muted {
    color: var(--muted);
  }
  .error {
    color: var(--danger);
  }
  .buttons {
    display: flex;
    gap: 0.5em;
    margin-top: 0.6em;
  }
  .cals {
    border-collapse: collapse;
    margin: 0.4em 0;
  }
  .cals th,
  .cals td {
    text-align: left;
    padding: 0.3em 1em 0.3em 0;
  }
  .on {
    display: block;
    margin-top: 0.6em;
  }
</style>
