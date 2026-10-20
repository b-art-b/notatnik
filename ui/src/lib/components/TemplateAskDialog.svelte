<script lang="ts">
  // The questions of a template (`{{ask:Label}}`, `{{ask:Label|default}}`) before its blocks are inserted.
  import { app } from '../stores/app.svelte'
  import { answerQuestions } from '../commands/asks'

  // `app.templateAsks` is set when the dialog opens and not null while it is shown: a snapshot is enough
  const ask = app.templateAsks ?? { name: '', asks: [] }
  // one value per question, starting with its default; `$state` makes the inputs follow it
  let values = $state(ask.asks.map((q) => q.default))
  let first: HTMLInputElement | undefined = $state()
  $effect(() => first?.focus())

  const done = () => answerQuestions(ask.asks.map((q, i) => ({ label: q.label, value: values[i] })))
  const cancel = () => answerQuestions(null)

  function key(e: KeyboardEvent) {
    e.stopPropagation() // keep Escape and Enter away from the editor and the global handler
    if (e.key === 'Escape') cancel()
  }
</script>

<div class="overlay" role="presentation" onclick={cancel}>
  <!-- the dialog is a div (a form cannot have this role); the form inside submits on Enter -->
  <div
    class="dialog"
    role="dialog"
    aria-label="Fill in the template"
    aria-modal="true"
    tabindex="-1"
    data-testid="template-ask"
    onclick={(e) => e.stopPropagation()}
    onkeydown={key}
  >
    <form
      onsubmit={(e) => {
        e.preventDefault()
        done()
      }}
    >
      <h2>{ask.name.replace(/^snippets\//, '')}</h2>
      {#each ask.asks as q, i (q.label)}
        <label>
          {q.label}
          <!-- `bind:this` only on the first input: it gets the focus -->
          {#if i === 0}
            <input bind:this={first} bind:value={values[i]} />
          {:else}
            <input bind:value={values[i]} />
          {/if}
        </label>
      {/each}
      <div class="row">
        <button type="submit">Insert</button>
        <button type="button" class="plain" onclick={cancel}>Cancel</button>
      </div>
    </form>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    justify-content: center;
    padding-top: 10vh;
    z-index: 50;
  }
  .dialog {
    width: min(420px, 92vw);
    max-height: 80vh;
    overflow: auto;
    background: var(--panel);
    border-radius: 10px;
    padding: 1em;
    align-self: flex-start;
  }
  h2 {
    margin: 0 0 0.6em;
    font-size: 1.1em;
  }
  label {
    display: block;
    margin-bottom: 0.6em;
  }
  input {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 0.5em;
    margin-top: 0.2em;
    font-size: 1em;
  }
  .row {
    display: flex;
    gap: 0.5em;
    margin-top: 0.8em;
  }
</style>
