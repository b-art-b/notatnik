<script lang="ts">
  // One setting: a title with a short explanation on the left, the control on the right. On a narrow window the
  // control wraps below the text. `labelFor` ties the title to a control that has an id.
  import type { Snippet } from 'svelte'

  let {
    title,
    description = '',
    labelFor,
    children,
  }: { title: string; description?: string; labelFor?: string; children: Snippet } = $props()
</script>

<div class="row">
  <div class="text">
    {#if labelFor}<label class="title" for={labelFor}>{title}</label>{:else}<div class="title">
        {title}
      </div>{/if}
    {#if description}<p class="desc">{description}</p>{/if}
  </div>
  <div class="control">{@render children()}</div>
</div>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.6em 2em;
    padding: 0.9em 0;
    border-bottom: 1px solid var(--border);
  }
  .row:last-child {
    border-bottom: none;
  }
  .text {
    flex: 1 1 18em;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    display: block;
  }
  .desc {
    margin: 0.15em 0 0;
    color: var(--muted);
    font-size: 0.85em;
  }
  .control {
    flex: 0 1 auto;
    max-width: 100%;
  }
</style>
