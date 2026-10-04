<script lang="ts">
  import { backend, type RenamePlanView } from '../api'
  import { navigate, toast, errorMessage } from '../stores/app.svelte'
  import { getDoc } from '../stores/pages.svelte'

  let { old, onclose }: { old: string; onclose: () => void } = $props()
  let name = $state(old)
  let plan = $state<RenamePlanView | null>(null)
  let error = $state('')

  async function preview() {
    error = ''
    plan = null
    try {
      plan = await backend.call('rename_plan', { old, new: name })
    } catch (e) {
      error = errorMessage(e)
    }
  }
  async function apply() {
    try {
      const p = await backend.call('rename_apply', { old, new: name })
      getDoc(p.name).view = p
      toast(`Renamed to ${p.name}`)
      onclose()
      navigate({ kind: 'page', name: p.name, zoom: [] })
    } catch (e) {
      error = errorMessage(e)
    }
  }
</script>

<div class="dialog" role="dialog" aria-label="Rename page" aria-modal="true">
  <label>New name <input bind:value={name} onkeydown={(e) => e.key === 'Enter' && preview()} /></label>
  <button onclick={preview}>Preview</button>
  <button onclick={onclose}>Cancel</button>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if plan}
    <div class="plan" data-testid="rename-plan">
      <p>
        {plan.edits.reduce((n, e) => n + e.replacements, 0)} link(s) in {plan.edits.length} file(s) will be rewritten{plan
          .file_moves.length
          ? `, and the page file moves to ${plan.file_moves[0][1]}`
          : ''}.
      </p>
      <ul>
        {#each plan.edits as e (e.path)}<li>{e.path} ({e.replacements})</li>{/each}
      </ul>
      {#each plan.warnings as w (w)}<p class="warn">{w}</p>{/each}
      <button class="primary" onclick={apply}>Rename</button>
    </div>
  {/if}
</div>

<style>
  .dialog {
    border: 1px solid var(--border);
    background: var(--panel);
    padding: 0.8em;
    border-radius: 8px;
    margin: 0.5em 0;
  }
  .error {
    color: var(--danger);
  }
  .warn {
    color: var(--warn);
  }
</style>
