<script lang="ts">
  import { openGraph } from '../stores/app.svelte'
  let path = $state('')
</script>

<div class="welcome">
  <h1>Notatnik</h1>
  <p>Open a folder of notes, or create a new graph.</p>
  <input bind:value={path} placeholder="/home/you/notes" aria-label="Graph folder" />
  <button onclick={() => openGraph(path, true)}>Open / create</button>
</div>

<style>
  .welcome {
    max-width: 480px;
    margin: 20vh auto;
  }
  input {
    width: 100%;
    padding: 0.5em;
    margin-bottom: 0.5em;
    box-sizing: border-box;
  }
</style>
