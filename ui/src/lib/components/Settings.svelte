<script lang="ts">
  import { backend, type SettingsView } from '../api'
  import { app, applyTheme, errorMessage, openGraph, toast, type Theme } from '../stores/app.svelte'
  import { setDateLocale } from '../dates'

  let s = $state<SettingsView | null>(null)
  let path = $state('')
  $effect(() => {
    backend.call('get_settings', {}).then((v) => (s = v))
  })

  async function save(
    change: Partial<{ theme: string; date_locale: string; week_start: string; completed_log_limit: number }>,
  ) {
    try {
      s = await backend.call('set_settings', {
        theme: null,
        date_locale: null,
        week_start: null,
        completed_log_limit: null,
        ...change,
      })
      setDateLocale(s.date_locale)
      applyTheme(s.theme as Theme)
      app.graph = await backend.call('graph_info', {})
      toast('Settings saved')
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }
</script>

<h1>Settings</h1>
{#if s}
  <section>
    <h2>This device</h2>
    <p class="muted">Stored in <code>~/.config/notatnik/settings.toml</code>, never synced.</p>
    <label
      >Theme
      <select value={s.theme} onchange={(e) => save({ theme: (e.currentTarget as HTMLSelectElement).value })}>
        <option value="system">Follow system</option><option value="light">Light</option><option value="dark"
          >Dark</option
        >
      </select>
    </label>
    <label
      >Date format
      <select
        value={s.date_locale}
        onchange={(e) => save({ date_locale: (e.currentTarget as HTMLSelectElement).value })}
      >
        <option value="en-GB">Sat, 3 Oct 2026 (en-GB)</option><option value="en-US"
          >Sat, Oct 3, 2026 (en-US)</option
        ><option value="pl-PL">sob., 3 paź 2026 (pl-PL)</option><option value="de-DE"
          >Sa., 3. Okt. 2026 (de-DE)</option
        >
      </select>
    </label>
  </section>
  <section>
    <h2>This graph</h2>
    <p class="muted">Stored in <code>.notatnik/config.toml</code> and synced with the notes.</p>
    <label
      >Week starts on
      <select
        value={s.week_start}
        onchange={(e) => save({ week_start: (e.currentTarget as HTMLSelectElement).value })}
      >
        <option value="monday">Monday</option><option value="sunday">Sunday</option>
      </select>
    </label>
    <label
      >Keep this many completion dates on repeating tasks (0 = all)
      <input
        type="number"
        min="0"
        value={s.completed_log_limit}
        onchange={(e) => save({ completed_log_limit: Number((e.currentTarget as HTMLInputElement).value) })}
      />
    </label>
    <p>Folder: <code>{s.graph_root}</code></p>
    <label>Open or create another graph <input bind:value={path} placeholder="/path/to/folder" /></label>
    <button onclick={() => openGraph(path, true)}>Open</button>
  </section>
{/if}

<style>
  label {
    display: block;
    margin: 0.6em 0;
  }
  section {
    margin-bottom: 1.5em;
  }
</style>
