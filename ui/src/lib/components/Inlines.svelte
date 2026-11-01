<script lang="ts">
  // Renders inline content from the backend's typed AST. There is no innerHTML anywhere: text is
  // always inserted as text, so raw HTML in notes shows up literally (and can't run).
  import { getContext } from 'svelte'
  import { backend, type Inline } from '../api'
  import { app } from '../stores/app.svelte'
  import { pageHash } from '../route'
  import { shownDate } from '../stores/format.svelte'
  import Self from './Inlines.svelte'
  import BlockRefView from './BlockRefView.svelte'
  import EmbedView from './EmbedView.svelte'
  import {
    DRAWING_HOST,
    drawingPath,
    openDrawing,
    type DrawingHost,
    type DrawingMode,
  } from '../commands/drawing'

  // the block this text is in, if any: it can rewrite or remove the reference to a drawing (`Block.svelte`)
  const drawingHost = getContext<DrawingHost | undefined>(DRAWING_HOST) ?? null

  /** The text of inline content, without its formatting (the name of a drawing written as a link). */
  function plainText(items: Inline[]): string {
    return items
      .map((x) =>
        x.type === 'text' || x.type === 'code'
          ? x.text
          : 'children' in x && Array.isArray(x.children)
            ? plainText(x.children)
            : '',
      )
      .join('')
  }

  /** A click on a drawing (a picture, or a link that is written as one): the editor opens on it with its name and
   * form; a new name or form rewrites the reference in the block (DR-6). */
  function editDrawing(path: string, name: string, mode: DrawingMode) {
    void openDrawing({ page: null, path, name, mode, host: drawingHost })
  }

  /** The address of an image; a drawing that was saved in this session gets a new one, so the cached picture is not shown. */
  function imageSrc(src: string): string {
    const url = backend.assetUrl(src)
    const drawing = drawingPath(src)
    const stamp = drawing ? app.drawingStamp[drawing] : undefined
    return stamp ? `${url}${url.includes('?') ? '&' : '?'}v=${stamp}` : url
  }

  let { items, chain = [] }: { items: Inline[]; chain?: string[] } = $props()

  function open(e: MouseEvent, name: string) {
    if (e.shiftKey) {
      e.preventDefault()
      if (!app.rightPanels.includes(name)) app.rightPanels.push(name)
    }
  }
  import PageIcon from './PageIcon.svelte'
</script>

{#each items as it, i (i)}
  {#if it.type === 'text'}{it.text}{:else if it.type === 'bold'}<strong
      ><Self items={it.children} {chain} /></strong
    >{:else if it.type === 'italic'}<em><Self items={it.children} {chain} /></em
    >{:else if it.type === 'strike'}<del><Self items={it.children} {chain} /></del
    >{:else if it.type === 'highlight'}<mark><Self items={it.children} {chain} /></mark
    >{:else if it.type === 'code'}<code>{it.text}</code>{:else if it.type === 'link'}{@const linked =
      drawingPath(it.href)}{#if linked}<!-- a drawing written as a link: a click opens the editor --><button
        type="button"
        class="drawing-link"
        title="Open the drawing"
        data-testid="drawing-link"
        onclick={(e) => {
          e.stopPropagation()
          editDrawing(linked, plainText(it.children), 'link')
        }}><span aria-hidden="true">✏️</span> <Self items={it.children} {chain} /></button
      >{:else}<a
        class="external"
        href={it.href}
        rel="noopener noreferrer"
        onclick={(e) => {
          e.preventDefault()
          e.stopPropagation()
          // a link without a scheme is a file copied into the graph (`../assets/…`)
          if (/^[a-z][a-z0-9+.-]*:/i.test(it.href)) backend.openExternal(it.href)
          else backend.openAsset(it.href)
        }}><Self items={it.children} {chain} /></a
      >{/if}{:else if it.type === 'image'}{@const drawing = drawingPath(
      it.src,
    )}{#if drawing}<!-- a drawing of this app: a click opens the editor on it (DR-6) --><button
        type="button"
        class="drawing"
        title={it.alt ? `Edit the drawing “${it.alt}”` : 'Edit the drawing'}
        aria-label={it.alt ? `Edit the drawing ${it.alt}` : 'Edit the drawing'}
        data-testid="drawing-image"
        onclick={(e) => {
          e.stopPropagation()
          editDrawing(drawing, it.alt, 'picture')
        }}><img class="inline-image" src={imageSrc(it.src)} alt={it.alt} /></button
      >{:else}<img
        class="inline-image"
        src={imageSrc(it.src)}
        alt={it.alt}
      />{/if}{:else if it.type === 'page_ref'}<a
      class="page-ref"
      href={pageHash(it.target)}
      data-page={it.target}
      onclick={(e) => {
        e.stopPropagation()
        open(e, it.target)
      }}
      >{#if it.label}<Self
          items={it.label}
          {chain}
        />{:else if /^\d{4}-\d{2}-\d{2}$/.test(it.target)}{shownDate(it.target)}{:else}<PageIcon
          name={it.target}
        />{it.target}{/if}</a
    >{:else if it.type === 'tag'}<a
      class="tag"
      href={pageHash(it.target)}
      data-page={it.target}
      onclick={(e) => {
        e.stopPropagation()
        open(e, it.target)
      }}>#{it.target}</a
    >{:else if it.type === 'block_ref'}<BlockRefView
      uid={it.uid}
      {chain}
    />{:else if it.type === 'block_embed'}<EmbedView
      kind="block"
      target={it.uid}
      {chain}
    />{:else if it.type === 'page_embed'}<EmbedView
      kind="page"
      target={it.target}
      {chain}
    />{:else if it.type === 'footnote_ref'}<sup class="footnote-ref" title={`Footnote ${it.label}`}
      >{it.label}</sup
    >{:else if it.type === 'soft_break' || it.type === 'hard_break'}<br />{/if}
{/each}
