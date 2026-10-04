<script lang="ts">
  import { backend, type SavedQueryView } from '../api'
  import { app, navigate } from '../stores/app.svelte'
  import QueryBlock from './QueryBlock.svelte'

  let { name }: { name: string } = $props()
  let q = $state<SavedQueryView | null | undefined>(undefined)
  $effect(() => {
    void app.savedVersion
    backend.call('list_saved_queries', {}).then((l) => (q = l.find((x) => x.name === name) ?? null))
  })
  async function remove() {
    await backend.call('delete_saved_query', { name })
    app.savedVersion++
    navigate({ kind: 'journals' })
  }
</script>

<h1>{name}</h1>
{#if q}
  {#key q.query}<QueryBlock text={q.query} saved />{/key}
  <button class="link" onclick={remove}>Remove from sidebar</button>
{:else if q === null}
  <p class="muted">No saved query called “{name}”.</p>
{/if}
