<script lang="ts">
  // A fenced code block as shown on the page, with a button that copies the code alone, without the fences (ED-28).
  // Svelte idiom: `$state` is the little piece of memory the button needs ("copied" for a moment); it lives in this
  // component because `Nodes.svelte` draws many code blocks and each needs its own.
  import { copyText } from '../commands/exec'

  let { code, lang }: { code: string; lang: string | null } = $props()
  let copied = $state(false)
  let timer: ReturnType<typeof setTimeout> | undefined

  async function copy(e: MouseEvent) {
    // a click on the page text opens the block for editing; this one must not
    e.stopPropagation()
    if (!(await copyText(code))) return
    copied = true
    clearTimeout(timer)
    timer = setTimeout(() => (copied = false), 1500)
  }
</script>

<div class="wrap">
  <pre class="code-block" data-lang={lang ?? ''}><code>{code}</code></pre>
  <button type="button" class="copy" class:done={copied} aria-label="Copy code" onclick={copy}
    >{copied ? 'Copied' : 'Copy'}</button
  >
</div>

<style>
  .wrap {
    position: relative;
  }
  pre.code-block {
    margin: 0.2em 0;
    padding: 0.5em 0.7em;
    background: var(--code-bg);
    border-radius: 6px;
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 0.88em;
    white-space: pre;
  }
  .copy {
    position: absolute;
    top: 0.45em;
    right: 0.5em;
    padding: 0.1em 0.5em;
    font-size: 0.75em;
    color: var(--muted);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
    opacity: 0;
  }
  /* shown on hover and keyboard focus; always on a touch screen, where there is no hover */
  .wrap:hover .copy,
  .copy:focus-visible,
  .copy.done {
    opacity: 1;
  }
  @media (hover: none) {
    .copy {
      opacity: 1;
    }
  }
</style>
