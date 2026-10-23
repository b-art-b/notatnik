<script lang="ts">
  // The padlock beside a page name (see `padlockOf`): closed = encrypted, open = kept plain. `testid` is the base
  // name; the open one adds `-open`, so a test can tell them apart. Size and spacing come from the place it is used.
  import type { PadlockMark } from '../stores/encryption.svelte'

  let {
    mark,
    testid,
    size = '0.8em',
    gap = '0.4em',
    middle = false,
  }: { mark: PadlockMark | null; testid: string; size?: string; gap?: string; middle?: boolean } = $props()
</script>

{#if mark}<span
    style:font-size={size}
    style:margin-left={gap}
    style:vertical-align={middle ? 'middle' : null}
    data-testid={mark.open ? `${testid}-open` : testid}
    role="img"
    aria-label={mark.title}
    title={mark.title}>{mark.open ? '🔓' : '🔒'}</span
  >{/if}
