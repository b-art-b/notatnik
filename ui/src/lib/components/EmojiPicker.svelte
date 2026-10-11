<script lang="ts">
  // Search box + grid of emojis (recent ones first). Used under the cursor in the editor (`/emoji`) and in Settings.
  // It takes the focus for the search box; the parent decides what happens to the focus after pick/cancel.
  import { onMount } from 'svelte'
  import { EMOJIS, EMOJI_GROUPS, recentEmoji, rememberEmoji, searchEmoji } from '../emoji'

  let { onpick, oncancel }: { onpick: (char: string) => void; oncancel: () => void } = $props()

  let q = $state('')
  let input: HTMLInputElement
  const recent = recentEmoji()
  const found = $derived(q.trim() ? searchEmoji(q, 80) : [])

  onMount(() => input.focus())

  function pick(char: string) {
    rememberEmoji(char)
    onpick(char)
  }
  function key(e: KeyboardEvent) {
    e.stopPropagation() // keep Enter/Escape away from the editor and the global handler
    if (e.key === 'Escape') {
      e.preventDefault()
      oncancel()
    } else if (e.key === 'Enter' && found.length > 0) {
      e.preventDefault()
      pick(found[0].char)
    }
  }
</script>

<!-- a click inside must not reach the block's own click handler (it would re-open the editor) -->
<div
  class="emoji-picker"
  role="dialog"
  aria-label="Pick an emoji"
  tabindex="-1"
  data-testid="emoji-picker"
  onkeydown={key}
  onclick={(e) => e.stopPropagation()}
>
  <input
    bind:this={input}
    bind:value={q}
    placeholder="Search emojis: smile, fire, check…"
    aria-label="Search emojis"
  />
  <div class="grid" role="listbox" aria-label="Emojis">
    {#if q.trim()}
      {#each found as e (e.char)}
        <button
          type="button"
          role="option"
          aria-selected="false"
          title={e.name}
          aria-label={e.name.replaceAll('_', ' ')}
          onclick={() => pick(e.char)}>{e.char}</button
        >
      {:else}
        <p class="none">No emoji found.</p>
      {/each}
    {:else}
      {#if recent.length}
        <h4>Recent</h4>
        <div class="row">
          {#each recent as c (c)}<button
              type="button"
              role="option"
              aria-selected="false"
              onclick={() => pick(c)}>{c}</button
            >{/each}
        </div>
      {/if}
      {#each EMOJI_GROUPS as g (g)}
        <h4>{g}</h4>
        <div class="row">
          {#each EMOJIS.filter((e) => e.group === g) as e (e.char)}
            <button
              type="button"
              role="option"
              aria-selected="false"
              title={e.name}
              onclick={() => pick(e.char)}>{e.char}</button
            >
          {/each}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .emoji-picker {
    display: flex;
    flex-direction: column;
    gap: 0.4em;
    padding: 0.6em;
    width: 21em;
    max-width: 90vw;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow:
      0 8px 28px rgb(0 0 0 / 0.18),
      0 1px 3px rgb(0 0 0 / 0.1);
    font-size: 0.9em;
  }
  .grid {
    max-height: 17em;
    overflow-y: auto;
  }
  h4 {
    margin: 0.5em 0 0.2em;
    font-size: 0.75em;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .row,
  .grid:has(> button) {
    display: flex;
    flex-wrap: wrap;
  }
  button {
    border: none;
    background: none;
    font-size: 1.35em;
    line-height: 1;
    padding: 0.18em;
    border-radius: 6px;
    cursor: pointer;
  }
  button:hover,
  button:focus-visible {
    background: var(--selection);
  }
  .none {
    color: var(--muted);
    margin: 0.5em 0;
  }
</style>
