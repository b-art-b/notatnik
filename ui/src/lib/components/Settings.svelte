<script lang="ts">
  import { app, applyTheme, openGraph, type Theme } from '../stores/app.svelte'
  let path = $state('')
</script>

<h1>Settings</h1>
<section>
  <h2>Appearance</h2>
  <label
    >Theme
    <select
      value={app.theme}
      onchange={(e) => applyTheme((e.currentTarget as HTMLSelectElement).value as Theme)}
    >
      <option value="system">Follow system</option><option value="light">Light</option><option value="dark"
        >Dark</option
      >
    </select>
  </label>
</section>
<section>
  <h2>Graph</h2>
  <p>Current: <code>{app.graph?.root}</code> (id {app.graph?.id}, weeks start on {app.graph?.week_start})</p>
  <label>Open or create a graph folder <input bind:value={path} placeholder="/path/to/folder" /></label>
  <button onclick={() => openGraph(path, true)}>Open</button>
</section>
