<script lang="ts">
  // The chain of follow-up tasks a task is in (TK-15): `/followup` marks a task done and creates the next one with
  // `follows:: ((id))`. This button opens the whole chain, first task to last; a task with several follow-ups
  // shows as a branch (indented). Loaded when opened, so a page full of chained tasks makes no extra requests.
  import { backend, type ChainView } from '../api'
  import { bus } from '../stores/events.svelte'
  import { jumpToBlock } from '../jump'
  import { errorMessage } from '../stores/app.svelte'

  let { page, path, compact = false }: { page: string; path: number[]; compact?: boolean } = $props()
  let open = $state(false)
  let chain = $state<ChainView | null>(null)
  let error = $state('')

  // reload while open, when the graph changes (a task of the chain was finished or edited)
  $effect(() => {
    void bus.version
    if (!open) return
    let cancelled = false
    backend.call('task_chain', { page, path }).then(
      (r) => {
        if (!cancelled) {
          chain = r
          error = ''
        }
      },
      (e) => {
        if (!cancelled) error = errorMessage(e)
      },
    )
    return () => {
      cancelled = true
    }
  })

  const glyph = (s: string | null) =>
    ({ DONE: '✓', CANCELED: '✕', DOING: '◐', NOW: '◐', WAITING: '◔' })[s ?? ''] ?? '☐'
  /** The block text without its task marker, priority and properties: what the task is about. */
  const label = (text: string) =>
    text
      .replace(/^(?:TODO|LATER|DOING|NOW|WAITING|DONE|CANCELED|\[[ xX]\])\s+/, '')
      .replace(/\s\^[a-z0-9]{6}$/, '')
      .trim() || '(empty)'
</script>

<!-- a click must not reach the block's own click handler (it would open the editor) -->
<span class="chain" data-testid="task-chain">
  <button
    type="button"
    class={compact ? 'ind' : 'chip'}
    data-chip="chain"
    aria-expanded={open}
    title="The chain of follow-up tasks this task is in"
    onclick={(e) => {
      e.stopPropagation()
      open = !open
    }}
    ><span aria-hidden="true">⛓</span>{#if !compact}&nbsp;chain{/if}</button
  >
  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="list" role="group" aria-label="Chain of tasks" onclick={(e) => e.stopPropagation()}>
      {#if error}<p class="problem">{error}</p>{/if}
      {#if chain}
        <p class="head" data-testid="chain-position">
          Task {chain.position + 1 - (chain.items[0]?.missing ? 1 : 0)} of {chain.items.filter(
            (i) => !i.missing,
          ).length}
        </p>
        <ol>
          {#each chain.items as it, i (i)}
            <li
              style:padding-left={`${it.depth * 1.2}em`}
              class:here={i === chain.position}
              data-testid="chain-item"
            >
              {#if it.missing}
                <span class="muted">a task that was followed is gone ((({it.uid})))</span>
              {:else}
                <span
                  class="st"
                  class:done={it.state === 'DONE' || it.state === 'CANCELED'}
                  aria-hidden="true">{glyph(it.state)}</span
                >
                <button type="button" class="open" onclick={() => jumpToBlock(it.page, it.path)}
                  >{label(it.text)}</button
                >
                <span class="where"
                  >{it.page}{it.deadline
                    ? ` · ${it.deadline}`
                    : it.scheduled
                      ? ` · ${it.scheduled}`
                      : ''}</span
                >
              {/if}
            </li>
          {/each}
        </ol>
      {:else if !error}
        <p class="muted">Loading…</p>
      {/if}
    </div>
  {/if}
</span>

<style>
  .chain {
    display: contents;
  }
  .chip,
  .ind {
    border: none;
    background: var(--chip);
    color: var(--muted);
    border-radius: 4px;
    padding: 0.05em 0.5em;
    cursor: pointer;
    font-size: 0.85em;
  }
  .list {
    flex-basis: 100%;
    margin: 0.3em 0;
    padding: 0.4em 0.6em;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    font-size: 0.9em;
  }
  .head,
  .muted,
  .where {
    color: var(--muted);
  }
  .head {
    margin: 0 0 0.3em;
    font-size: 0.85em;
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    gap: 0.5em;
    align-items: baseline;
    padding: 0.1em 0;
  }
  li.here .open {
    font-weight: 600;
  }
  .st.done {
    color: var(--muted);
  }
  .open {
    border: none;
    background: none;
    padding: 0;
    color: var(--link);
    cursor: pointer;
    text-align: left;
  }
  .problem {
    color: var(--danger);
  }
</style>
