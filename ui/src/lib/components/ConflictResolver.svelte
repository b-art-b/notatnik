<script lang="ts">
  // Resolving one conflict copy (ST-6): the page and its copy are merged with the page as it was before they
  // diverged. Where only one side changed a block the merge takes that change. What both changed is decided here,
  // block by block. Nothing is written until "Write the merge".
  import { backend, type ConflictMergeView } from '../api'
  import { errorMessage, refreshConflicts, toast } from '../stores/app.svelte'
  import { openDocs } from '../stores/pages.svelte'
  import { changed, diffLines } from '../diff'

  let { path, onclose }: { path: string; onclose: () => void } = $props()

  type Pick = 'ours' | 'theirs' | 'both' | 'text'
  type Choice = { pick: Pick; text: string }

  let view = $state<ConflictMergeView | null>(null)
  // one decision per conflict, in the order the backend lists them
  let choices = $state<Choice[]>([])
  let busy = $state(false)

  /** What the backend should do with the choices: `text` is only sent for an edited block. */
  const sent = () => choices.map((c) => ({ pick: c.pick, text: c.pick === 'text' ? c.text : null }))

  async function load() {
    view = await backend.call('conflict_merge', { path, choices: [] })
    // a block deleted on one side and edited on the other is kept unless you say otherwise
    choices = view.conflicts.map((c) => ({
      pick: c.kind === 'edit_delete' ? 'both' : 'ours',
      text: c.ours ?? c.theirs ?? '',
    }))
  }

  // `$effect` runs after the component is shown and again when what it reads changes: `path` here
  $effect(() => {
    void path
    load().catch((e) => toast(errorMessage(e), 'error'))
  })

  /** Shows the merged page for the current choices. */
  async function preview() {
    try {
      view = await backend.call('conflict_merge', { path, choices: sent() })
    } catch (e) {
      toast(errorMessage(e), 'error')
    }
  }

  const diff = $derived(
    view?.original_text != null && view.merged_text != null
      ? diffLines(view.original_text, view.merged_text)
      : [],
  )

  async function resolve(mode: 'merge' | 'keep_original' | 'use_copy') {
    if (!view) return
    busy = true
    try {
      await backend.call('resolve_conflict', {
        path,
        mode,
        choices: sent(),
        expected_hash: view.original_hash,
      })
      // the page may be open: read it again
      for (const d of openDocs()) if (d.name === view.original) void d.load()
      await refreshConflicts()
      toast(
        mode === 'merge'
          ? 'Merged. The copy is in the trash folder.'
          : 'Done. The copy is in the trash folder.',
      )
      onclose()
    } catch (e) {
      toast(errorMessage(e), 'error')
    } finally {
      busy = false
    }
  }

  const KIND: Record<string, string> = {
    edit: 'Both changed this block',
    edit_delete: 'One side deleted this block, the other changed it',
    text: 'Both changed the text around the blocks',
  }
</script>

<div class="dialog" role="dialog" aria-label="Resolve conflict" data-testid="conflict-resolver">
  {#if !view}
    <p class="muted">Loading…</p>
  {:else}
    <p>
      <strong>Conflict in {view.original ?? path}</strong>
      <span class="muted">
        {#if view.base === 'synced'}
          Compared with the page as it was at the last sync.
        {:else if view.base === 'history'}
          No common ancestor was kept; one was guessed from this page's history, so check the result.
        {:else}
          No common ancestor is known, so the two versions cannot be merged. Choose one.
        {/if}
      </span>
    </p>

    {#if view.base === 'none'}
      <div class="cols">
        <div>
          <h4>The page</h4>
          <pre>{view.original_text ?? '(not found)'}</pre>
        </div>
        <div>
          <h4>The copy</h4>
          <pre>{view.copy_text}</pre>
        </div>
      </div>
    {:else}
      {#if view.clean}
        <p data-testid="conflict-clean">Nothing to decide: the changes do not overlap.</p>
      {/if}
      {#each view.conflicts as c, i (c.index)}
        <fieldset class="conflict" data-testid="conflict-block">
          <legend>{KIND[c.kind] ?? 'Conflict'}: <em>{c.label || '(empty)'}</em></legend>
          <div class="cols three">
            <div>
              <h4>Before</h4>
              <pre>{c.base ?? '(did not exist)'}</pre>
            </div>
            <div>
              <h4>The page</h4>
              <pre>{c.ours ?? '(deleted)'}</pre>
            </div>
            <div>
              <h4>The copy</h4>
              <pre>{c.theirs ?? '(deleted)'}</pre>
            </div>
          </div>
          <div class="picks" role="radiogroup" aria-label="Which to keep">
            <!-- `bind:group` keeps the chosen value in `choices[i].pick`; onchange then asks for a new preview -->
            {#each [['ours', c.ours === null ? 'Delete it' : 'The page'], ['theirs', c.theirs === null ? 'Delete it' : 'The copy'], ['both', c.ours === null || c.theirs === null ? 'Keep it' : 'Both'], ['text', 'Edit…']] as [value, label] (value)}
              <label>
                <input
                  type="radio"
                  name={`pick-${i}`}
                  {value}
                  bind:group={choices[i].pick}
                  onchange={preview}
                />
                {label}
              </label>
            {/each}
          </div>
          {#if choices[i]?.pick === 'text'}
            <textarea aria-label="Edited block" rows="4" bind:value={choices[i].text} onchange={preview}
            ></textarea>
          {/if}
        </fieldset>
      {/each}
      <p class="muted">
        {changed(diff)
          ? 'Writing the merge changes the page like this (− now, + merged):'
          : 'The merge equals the page now.'}
      </p>
      {#if changed(diff)}
        <pre class="diff" data-testid="conflict-diff">{#each diff as l, i (i)}<span class={l.kind}
              >{l.kind === 'add' ? '+ ' : l.kind === 'del' ? '− ' : '  '}{l.text}
</span>{/each}</pre>
      {/if}
    {/if}

    <div class="buttons">
      {#if view.base !== 'none'}
        <button disabled={busy} onclick={() => resolve('merge')} data-testid="conflict-merge"
          >Write the merge</button
        >
      {/if}
      <button
        class="plain"
        disabled={busy || view.original_text == null}
        onclick={() => resolve('keep_original')}
        data-testid="conflict-keep">Keep the page, drop the copy</button
      >
      <button
        class="plain"
        disabled={busy}
        onclick={() => resolve('use_copy')}
        data-testid="conflict-use-copy">Use the copy as the page</button
      >
      <button class="plain" onclick={onclose}>Close</button>
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
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.8em;
  }
  .cols.three {
    grid-template-columns: repeat(3, 1fr);
  }
  h4 {
    margin: 0 0 0.2em;
    font-size: 0.85em;
    color: var(--muted);
  }
  pre {
    white-space: pre-wrap;
    background: var(--code-bg);
    padding: 0.4em;
    margin: 0;
    border-radius: 6px;
    font-size: 0.85em;
    overflow-wrap: anywhere;
  }
  .conflict {
    border: 1px solid var(--border);
    border-radius: 6px;
    margin: 0.6em 0;
  }
  .picks {
    display: flex;
    gap: 1em;
    margin-top: 0.5em;
    flex-wrap: wrap;
  }
  textarea {
    width: 100%;
    margin-top: 0.4em;
    font-family: monospace;
  }
  .diff {
    max-height: 18em;
    overflow: auto;
  }
  .diff span {
    display: block;
  }
  .add {
    background: color-mix(in srgb, #2e9d4f 18%, transparent);
  }
  .del {
    background: color-mix(in srgb, var(--danger) 18%, transparent);
  }
  .buttons {
    display: flex;
    gap: 0.5em;
    flex-wrap: wrap;
    margin-top: 0.7em;
  }
  .muted {
    color: var(--muted);
    font-size: 0.85em;
  }
</style>
