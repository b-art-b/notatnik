<script lang="ts">
  import { backend, CommandError, type RenamePlanView } from '../api'
  import { navigate, toast, errorMessage } from '../stores/app.svelte'
  import { getDoc, reloadOpenDocs } from '../stores/pages.svelte'
  import { renameRecent, renameRecentBelow } from '../stores/recent.svelte'
  import { prefixRename, underPrefix } from '../rename'

  let { old, onclose }: { old: string; onclose: () => void } = $props()
  let name = $state(old)
  let plan = $state<RenamePlanView | null>(null)
  let error = $state('')
  // set when the move stopped half way (a file changed on disk): the user can revert what was written
  let stopped = $state(false)
  // a page with subpages moves together with them, unless you choose to move only this page (which is also the
  // only way to give it a name below its own: `a` -> `a/b`)
  const hasSubpages = $derived((getDoc(old).view?.child_pages.length ?? 0) > 0)
  let withSub = $state(true)
  const inside = $derived(
    name
      .trim()
      .toLowerCase()
      .startsWith(old.toLowerCase() + '/'),
  )
  const includeSubpages = $derived(hasSubpages ? withSub && !inside : undefined)
  // `P/CapGemini/sth` -> `P/Capgemini12/sth`: by default this renames the namespace `P/CapGemini`, so the pages
  // beside it follow; unticking moves only this page
  const prefix = $derived(prefixRename(old, name.trim()))
  let allBelow = $state(true)
  const asNamespace = $derived(prefix !== null && allBelow)
  // what is sent to the backend
  const req = $derived(
    asNamespace && prefix
      ? { old: prefix.from, new: prefix.to, include_subpages: true }
      : { old, new: name, include_subpages: includeSubpages ?? null },
  )

  async function preview() {
    error = ''
    plan = null
    try {
      plan = await backend.call('rename_plan', req)
    } catch (e) {
      error = errorMessage(e)
    }
  }
  async function apply() {
    try {
      const p = await backend.call('rename_apply', req)
      getDoc(p.name).view = p
      void reloadOpenDocs() // links in other open pages (a journal) were rewritten
      // stay on the page that was being renamed (for a namespace rename the result is the namespace page)
      const here = asNamespace && prefix ? underPrefix(old, prefix) : p.name
      if (asNamespace && prefix) renameRecentBelow(prefix.from, prefix.to)
      else renameRecent(old, p.name)
      toast(`Renamed to ${asNamespace && prefix ? prefix.to : p.name}`)
      onclose()
      navigate({ kind: 'page', name: here, zoom: [] })
    } catch (e) {
      error = errorMessage(e)
      stopped = e instanceof CommandError && e.code === 'conflict'
    }
  }
  async function revert() {
    try {
      const r = await backend.call('rename_revert', {})
      stopped = false
      plan = null
      error = ''
      void reloadOpenDocs()
      toast(`Reverted ${r.items.length} file(s)`)
    } catch (e) {
      error = errorMessage(e)
    }
  }
</script>

<div class="dialog" role="dialog" aria-label="Rename page" aria-modal="true">
  <label>New name <input bind:value={name} onkeydown={(e) => e.key === 'Enter' && preview()} /></label>
  {#if prefix}
    <label class="subpages">
      <input type="checkbox" bind:checked={allBelow} onchange={() => (plan = null)} />
      Rename <strong>{prefix.from}</strong> to <strong>{prefix.to}</strong> for all pages below it
      {#if !allBelow}<span class="muted">(only this page moves)</span>{/if}
    </label>
  {/if}
  {#if hasSubpages && !asNamespace}
    <label class="subpages">
      <input
        type="checkbox"
        checked={withSub && !inside}
        disabled={inside}
        onchange={(e) => (withSub = (e.currentTarget as HTMLInputElement).checked)}
      />
      Move the subpages too
      {#if inside}<span class="muted"
          >(not possible here: the name is inside this page, so only this page moves)</span
        >{/if}
    </label>
  {/if}
  <button onclick={preview}>Preview</button>
  <button onclick={onclose}>Cancel</button>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if stopped}
    <p>
      The move stopped half way. <strong>Revert</strong> restores the files already rewritten; or press
      <strong>Preview</strong> and <strong>Rename</strong> again to finish it.
      <button onclick={revert}>Revert</button>
    </p>
  {/if}
  {#if plan}
    <div class="plan" data-testid="rename-plan">
      <p>
        {plan.edits.reduce((n, e) => n + e.replacements, 0)} link(s) in {plan.edits.length} file(s) will be rewritten{plan
          .file_moves.length === 1
          ? `, and the page file moves to ${plan.file_moves[0][1]}`
          : plan.file_moves.length > 1
            ? `, and ${plan.file_moves.length} page files move`
            : ''}.
      </p>
      {#if plan.file_moves.length > 1}
        <details data-testid="rename-moves">
          <summary>Files that move</summary>
          <ul>
            {#each plan.file_moves as m (m[0])}<li>{m[0]} → {m[1]}</li>{/each}
          </ul>
        </details>
      {/if}
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
