<script lang="ts">
  // The small button that opens the document at a highlight: beside a `((id))` reference, and on the highlights of a
  // sidecar page (PA-6, EA-7).
  import { openHighlight, type HighlightDoc } from '../annotate/highlightDoc'

  let { doc, uid, label }: { doc: HighlightDoc; uid: string; label?: string } = $props()
</script>

<button
  type="button"
  class="pdf-chip"
  data-testid="pdf-ref-chip"
  title="Open {doc.name} at this highlight"
  onclick={(e) => {
    e.stopPropagation()
    openHighlight(doc, uid)
  }}>{label ?? doc.label}</button
>

<style>
  .pdf-chip {
    font: inherit;
    font-size: 0.75em;
    margin-left: 0.4em;
    padding: 0 0.5em;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--chip);
    color: var(--muted);
    cursor: pointer;
    white-space: nowrap;
  }
  .pdf-chip:hover {
    color: inherit;
    border-color: var(--accent);
  }
  @media (pointer: coarse) {
    .pdf-chip {
      min-height: 44px;
    }
  }
</style>
