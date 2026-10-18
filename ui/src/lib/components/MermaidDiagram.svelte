<script lang="ts">
  // A ```mermaid block drawn as a picture. The library is big, so it is loaded the first time a
  // diagram is shown. The SVG is shown through an <img> (a data URL): pictures can't run scripts,
  // so no innerHTML is needed. `htmlLabels: false` keeps the text inside the SVG (an <img> can't
  // draw HTML labels).
  let { code }: { code: string } = $props()

  let src = $state('')
  let failed = $state('')
  let seq = 0

  // `$effect` re-runs when `code` changes. A newer run makes an older one stop (`seq`).
  $effect(() => {
    const text = code
    const mine = ++seq
    ;(async () => {
      try {
        const { default: mermaid } = await import('mermaid')
        const dark = document.documentElement.dataset.theme === 'dark'
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: 'strict',
          theme: dark ? 'dark' : 'default',
          flowchart: { htmlLabels: false },
        })
        const { svg } = await mermaid.render(`mmd-${mine}-${Math.random().toString(36).slice(2)}`, text)
        if (mine !== seq) return
        src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg)
        failed = ''
      } catch (e) {
        if (mine !== seq) return
        src = ''
        failed = e instanceof Error ? e.message.split('\n')[0] : 'Could not draw the diagram'
      }
    })()
  })
</script>

{#if failed}
  <pre class="code-block" data-lang="mermaid"><code>{code}</code></pre>
  <p class="mermaid-error" role="status">Diagram not drawn: {failed}</p>
{:else if src}
  <img class="mermaid" {src} alt="Mermaid diagram" />
{:else}
  <p class="mermaid-wait">Drawing diagram…</p>
{/if}

<style>
  .mermaid {
    max-width: 100%;
    margin: 0.2em 0;
  }
  .mermaid-wait,
  .mermaid-error {
    margin: 0.2em 0;
    color: var(--muted);
    font-size: 0.85em;
  }
  pre.code-block {
    margin: 0.2em 0;
    padding: 0.5em 0.7em;
    background: var(--code-bg);
    border-radius: 6px;
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 0.88em;
  }
</style>
