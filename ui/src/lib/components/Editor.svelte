<script lang="ts">
  // One CodeMirror 6 instance, mounted on the focused block only (ED-2). It edits the block's raw
  // Markdown. Svelte idiom: `onMount` creates the view, and the returned function destroys it.
  import { onMount, untrack } from 'svelte'
  import { editing } from '../stores/editing.svelte'
  import { EditorState, Prec, type Extension } from '@codemirror/state'
  import {
    Decoration,
    EditorView,
    keymap,
    MatchDecorator,
    ViewPlugin,
    type DecorationSet,
    type ViewUpdate,
  } from '@codemirror/view'
  import {
    defaultKeymap,
    history,
    historyKeymap,
    isolateHistory,
    cursorLineEnd,
    cursorLineStart,
    selectLineEnd,
    selectLineStart,
    insertNewline,
    deleteCharBackward,
    deleteCharForward,
    redoDepth,
    undoDepth,
    undo as cmUndo,
    redo as cmRedo,
  } from '@codemirror/commands'
  import { acceptCompletion, closeCompletion, completionStatus } from '@codemirror/autocomplete'
  import { markdown } from '@codemirror/lang-markdown'
  import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language'
  import type { Cursor } from '../tree'
  import type { EditorAction } from '../commands/exec'
  import { wikiCompletions } from '../completion'
  import { backend } from '../api'
  import { app, toast } from '../stores/app.svelte'
  import { fileLink } from '../filelinks'
  import { smartPaste } from '../pastelink'
  import { hasFiles, imageFiles, pngFromBase64, toBase64, usefulName } from '../images'
  import { dateChanges, DATE_TITLES, refreshTemplateNames, type DateKind } from '../commands/slash'
  import DatePicker from './DatePicker.svelte'
  import EmojiPicker from './EmojiPicker.svelte'
  import TemplatePicker from './TemplatePicker.svelte'
  import FilePicker from './FilePicker.svelte'

  interface Props {
    text: string
    cursor: Cursor
    /** Current page name (completions rank its namespace first). */
    pageName: string
    /** Called ~500 ms after the last keystroke, and when the block loses focus. */
    onchange: (text: string, prev: string) => void
    onaction: (a: EditorAction) => Promise<boolean>
    onundo: (kind: 'undo' | 'redo') => void
    onpaste: (text: string) => Promise<boolean>
    onblur: (text: string, prev: string) => void
    /** Text between bullets: Enter adds a line, Tab and merging with the neighbours are off, a multi-line paste
     * stays one piece of text. */
    plain?: boolean
  }
  let { text, cursor, pageName, onchange, onaction, onundo, onpaste, onblur, plain = false }: Props = $props()

  let host: HTMLDivElement
  let view: EditorView | undefined
  let idle: ReturnType<typeof setTimeout> | undefined
  // the text the editor starts with; updated as changes are sent (not reactive on purpose)
  let lastSent = untrack(() => text)
  /** The open date picker (from /date, /scheduled, /deadline), positioned under the cursor. */
  let picker = $state<{
    kind: DateKind | 'emoji' | 'file' | 'template'
    at: number
    x: number
    y: number
  } | null>(null)

  /** Stores pasted or dropped images as assets (ST-8) and writes their links into the text: replacing the
   * selection, or at `at` for a drop. The editor may be gone when the backend answers; then only the files remain. */
  async function addImages(files: File[], at?: number) {
    const v = view
    if (!v) return
    const links: string[] = []
    for (const f of files) {
      try {
        const saved = await backend.call('save_asset', {
          page: pageName,
          data: await toBase64(f),
          name: usefulName(f),
        })
        links.push(saved.markdown)
      } catch (e) {
        toast(e instanceof Error ? e.message : String(e), 'error')
      }
    }
    if (view !== v || links.length === 0) return
    insertLinks(v, links, at)
  }

  /** Looks up the title of the web address that was just pasted and turns the bare address into `[title](address)`.
   * Nothing happens when the lookup fails, the editor is gone or the address has been edited meanwhile. */
  async function addTitle(v: EditorView, url: string, from: number) {
    let title = ''
    try {
      title = (await backend.call('link_title', { url })).value
    } catch {
      return // offline, refused, not a page: the address stays as it is
    }
    if (!title || view !== v) return
    // where it was pasted, if the text before it did not change; else the first bare copy of the address
    const doc = v.state.doc.toString()
    const bare = (i: number) => !/\]\($|<$/.test(doc.slice(Math.max(0, i - 2), i))
    let at = doc.startsWith(url, from) && bare(from) ? from : doc.indexOf(url)
    while (at >= 0 && !bare(at)) at = doc.indexOf(url, at + 1)
    if (at < 0) return
    const insert = fileLink(title, url)
    // its own undo step: one Ctrl+Z gives the plain address back
    v.dispatch({
      changes: { from: at, to: at + url.length, insert },
      userEvent: 'input.paste',
      annotations: isolateHistory.of('full'),
    })
  }

  /** Writes image links at the selection (or at `at`), with a space in front if they would stick to a word. */
  function insertLinks(v: EditorView, links: string[], at?: number) {
    const sel = v.state.selection.main
    const from = at ?? sel.from
    const to = at ?? sel.to
    // a space before the link when it would stick to the word in front (`#tag![](...)` is not an image)
    const glued = from > 0 && !/\s/.test(v.state.sliceDoc(from - 1, from))
    const insert = (glued ? ' ' : '') + links.join('\n')
    v.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + insert.length },
      userEvent: 'input.paste',
    })
    v.focus()
  }

  function openPicker(kind: DateKind | 'emoji' | 'file' | 'template', at: number) {
    if (!view) return
    const c = view.coordsAtPos(at)
    const h = host.getBoundingClientRect()
    picker = { kind, at, x: c ? c.left - h.left : 0, y: c ? c.bottom - h.top + 4 : 24 }
  }

  function closePicker() {
    picker = null
    view?.focus()
  }

  /** Escape in the emoji picker: drop the space that was typed before `/emoji` if nothing follows it. */
  function cancelEmoji() {
    if (
      picker &&
      view &&
      picker.at === view.state.doc.length &&
      view.state.sliceDoc(picker.at - 1, picker.at) === ' '
    ) {
      view.dispatch({ changes: { from: picker.at - 1, to: picker.at, insert: '' } })
    }
    closePicker()
  }

  function pickEmoji(char: string) {
    if (!picker || !view) return
    const { at } = picker
    view.dispatch({ changes: { from: at, to: at, insert: char }, selection: { anchor: at + char.length } })
    closePicker()
  }

  function pickDate(iso: string) {
    if (!picker || !view || picker.kind === 'emoji' || picker.kind === 'file' || picker.kind === 'template')
      return
    const { kind, at } = picker
    view.dispatch({ changes: dateChanges(view.state.doc.toString(), at, kind, iso) })
    closePicker()
  }

  /** A template from the list: its blocks are inserted after this block, like `/template-<name>` always did. */
  function pickTemplate(template: string) {
    if (!view) return
    closePicker()
    clearTimeout(idle)
    void onaction({ kind: 'template', name: template, text: view.state.doc.toString(), prev: lastSent })
  }

  /** A file link from the picker, written where `/file` was typed. */
  function pickFile(markdown: string) {
    if (!picker || !view) return
    const { at } = picker
    picker = null
    insertLinks(view, [markdown], at)
  }

  const wiki = new MatchDecorator({
    regexp:
      /\[\[[^\]\n]*\]\]|(?<=^|[\s(])#\[\[[^\]\n]*\]\]|(?<=^|[\s(])#[\p{L}\p{N}_/-]+|\(\([a-z0-9]{6}\)\)|\^[a-z0-9]{6}$|==[^=\n]+==/gu,
    decoration: (m) => {
      const s = m[0]
      const cls = s.startsWith('[[')
        ? 'cm-wikilink'
        : s.startsWith('#')
          ? 'cm-tag'
          : s.startsWith('((')
            ? 'cm-blockref'
            : s.startsWith('==')
              ? 'cm-hl'
              : 'cm-blockid'
      return Decoration.mark({ class: cls })
    },
  })
  const wikiPlugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet
      constructor(v: EditorView) {
        this.decorations = wiki.createDeco(v)
      }
      update(u: ViewUpdate) {
        this.decorations = wiki.updateDeco(u, this.decorations)
      }
    },
    { decorations: (p) => p.decorations },
  )

  function currentText(): string {
    return view ? view.state.doc.toString() : text
  }

  function flushNow() {
    clearTimeout(idle)
    const t = currentText()
    if (t !== lastSent) {
      const prev = lastSent
      lastSent = t
      onchange(t, prev)
    }
  }

  function lineInfo(v: EditorView) {
    const head = v.state.selection.main.head
    const line = v.state.doc.lineAt(head)
    return {
      head,
      line,
      first: line.number === 1,
      last: line.number === v.state.doc.lines,
      column: head - line.from,
    }
  }

  const act = (v: EditorView, build: (t: string) => EditorAction) => {
    clearTimeout(idle)
    void onaction(build(v.state.doc.toString()))
    return true
  }

  /** The keys of a plain text block that differ from a bullet's: the text is not an outline item. */
  const plainKeys = () => [
    {
      key: 'Enter',
      run: (v: EditorView) =>
        completionStatus(v.state) === 'active' ? acceptCompletion(v) : insertNewline(v),
    },
    { key: 'Backspace', run: deleteCharBackward },
    { key: 'Delete', run: deleteCharForward },
    ...[
      'Tab',
      'Shift-Tab',
      'Shift-ISO_Left_Tab',
      'ISO_Left_Tab',
      'Alt-ArrowRight',
      'Alt-ArrowLeft',
      'Mod-Enter',
    ].map((key) => ({ key, run: () => true })),
  ]

  function keys(): Extension {
    return Prec.highest(
      keymap.of([
        ...(plain ? plainKeys() : []),
        {
          key: 'Enter',
          run: (v) => {
            if (completionStatus(v.state) === 'active') return acceptCompletion(v)
            const { head } = lineInfo(v)
            if (v.state.selection.main.from !== v.state.selection.main.to) return false
            // run the split; if it declines (cursor not on the first line, in a fence) insert a newline
            clearTimeout(idle)
            void onaction({ kind: 'enter', text: v.state.doc.toString(), prev: lastSent, cursor: head }).then(
              (done) => {
                if (!done) insertNewline(v)
              },
            )
            return true
          },
        },
        { key: 'Shift-Enter', run: insertNewline },
        // Home / End go to the start / end of the whole line, also in text that wraps over several rows on screen
        // (CodeMirror's own keys stop at the row)
        { key: 'End', run: cursorLineEnd, shift: selectLineEnd },
        { key: 'Home', run: cursorLineStart, shift: selectLineStart },
        {
          // ← at the very start of a block goes to the end of the block before it, → at the very end to the start
          // of the next one (a collapsed block's children are skipped)
          key: 'ArrowLeft',
          run: (v) => {
            const s = v.state.selection.main
            if (completionStatus(v.state) === 'active' || !s.empty || s.head !== 0) return false
            clearTimeout(idle)
            void onaction({ kind: 'nav-left', text: v.state.doc.toString(), prev: lastSent })
            return true
          },
        },
        {
          key: 'ArrowRight',
          run: (v) => {
            const s = v.state.selection.main
            if (completionStatus(v.state) === 'active' || !s.empty || s.head !== v.state.doc.length)
              return false
            clearTimeout(idle)
            void onaction({ kind: 'nav-right', text: v.state.doc.toString(), prev: lastSent })
            return true
          },
        },
        {
          // Ctrl+A selects the text of the block; pressed again (all of it is selected, or the block is empty) it
          // selects every block of the page
          key: 'Mod-a',
          run: (v) => {
            const s = v.state.selection.main
            if (s.from !== 0 || s.to !== v.state.doc.length) return false
            clearTimeout(idle)
            void onaction({ kind: 'select-all', text: v.state.doc.toString(), prev: lastSent })
            return true
          },
        },
        { key: 'Mod-Enter', run: (v) => act(v, (t) => ({ kind: 'cycle-task', text: t, prev: lastSent })) },
        { key: 'Tab', run: (v) => act(v, (t) => ({ kind: 'indent', text: t, prev: lastSent })) },
        { key: 'Shift-Tab', run: (v) => act(v, (t) => ({ kind: 'outdent', text: t, prev: lastSent })) },
        // WebKitGTK (the desktop app on Linux) names Shift+Tab `ISO_Left_Tab`
        {
          key: 'Shift-ISO_Left_Tab',
          run: (v) => act(v, (t) => ({ kind: 'outdent', text: t, prev: lastSent })),
        },
        { key: 'ISO_Left_Tab', run: (v) => act(v, (t) => ({ kind: 'outdent', text: t, prev: lastSent })) },
        // like org-mode's M-<right> / M-<left>: demote / promote the block (with its children)
        { key: 'Alt-ArrowRight', run: (v) => act(v, (t) => ({ kind: 'indent', text: t, prev: lastSent })) },
        { key: 'Alt-ArrowLeft', run: (v) => act(v, (t) => ({ kind: 'outdent', text: t, prev: lastSent })) },
        { key: 'Alt-ArrowUp', run: (v) => act(v, (t) => ({ kind: 'move-up', text: t, prev: lastSent })) },
        { key: 'Alt-ArrowDown', run: (v) => act(v, (t) => ({ kind: 'move-down', text: t, prev: lastSent })) },
        {
          key: 'ArrowUp',
          run: (v) => {
            if (completionStatus(v.state) === 'active') return false
            const i = lineInfo(v)
            if (!i.first) return false
            clearTimeout(idle)
            void onaction({ kind: 'nav-up', text: v.state.doc.toString(), prev: lastSent, column: i.column })
            return true
          },
        },
        {
          key: 'ArrowDown',
          run: (v) => {
            if (completionStatus(v.state) === 'active') return false
            const i = lineInfo(v)
            if (!i.last) return false
            clearTimeout(idle)
            void onaction({
              kind: 'nav-down',
              text: v.state.doc.toString(),
              prev: lastSent,
              column: i.column,
            })
            return true
          },
        },
        {
          key: 'Backspace',
          run: (v) => {
            const s = v.state.selection.main
            if (s.from !== 0 || s.to !== 0) return false
            clearTimeout(idle)
            void onaction({ kind: 'merge-prev', text: v.state.doc.toString(), prev: lastSent })
            return true
          },
        },
        {
          key: 'Delete',
          run: (v) => {
            const s = v.state.selection.main
            if (s.from !== v.state.doc.length || s.to !== v.state.doc.length) return false
            clearTimeout(idle)
            void onaction({ kind: 'merge-next', text: v.state.doc.toString(), prev: lastSent })
            return true
          },
        },
        {
          key: 'Escape',
          run: (v) =>
            completionStatus(v.state) === 'active'
              ? closeCompletion(v)
              : act(v, (t) => ({ kind: 'escape', text: t, prev: lastSent })),
        },
        {
          key: 'Mod-z',
          run: (v) => {
            if (undoDepth(v.state) > 0) return cmUndo(v)
            flushNow()
            onundo('undo')
            return true
          },
        },
        {
          key: 'Mod-Shift-z',
          mac: 'Mod-Shift-z',
          run: (v) => {
            if (redoDepth(v.state) > 0) return cmRedo(v)
            flushNow()
            onundo('redo')
            return true
          },
        },
        {
          key: 'Mod-y',
          run: (v) => {
            if (redoDepth(v.state) > 0) return cmRedo(v)
            flushNow()
            onundo('redo')
            return true
          },
        },
      ]),
    )
  }

  onMount(() => {
    void refreshTemplateNames()
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          keys(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          // pasting an address is handled below (ED-27); the built-in version would not clean it
          markdown({ pasteURLAsLink: false }),
          syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
          wikiPlugin,
          wikiCompletions(
            () => pageName,
            openPicker,
            (at) => openPicker('emoji', at),
            (name) => {
              if (!view) return
              clearTimeout(idle)
              void onaction({ kind: 'template', name, text: view.state.doc.toString(), prev: lastSent })
            },
            () => {
              if (!view) return
              clearTimeout(idle)
              void onaction({ kind: 'followup', text: view.state.doc.toString(), prev: lastSent })
            },
            (at) => openPicker('file', at),
            (at) => openPicker('template', at),
          ),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ 'aria-label': 'Block text', 'data-testid': 'block-editor' }),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) {
              clearTimeout(idle)
              idle = setTimeout(flushNow, 500)
            }
          }),
          EditorView.domEventHandlers({
            blur: () => {
              queueMicrotask(() => {
                // a popup (autocomplete) taking focus is not leaving the block
                if (view && !view.hasFocus && !picker && !document.activeElement?.closest('.cm-tooltip')) {
                  clearTimeout(idle)
                  const t = currentText()
                  const prev = lastSent
                  lastSent = t
                  onblur(t, prev)
                }
              })
              return false
            },
            dragover: (e) => {
              if (!hasFiles(e.dataTransfer)) return false
              e.preventDefault()
              return true
            },
            drop: (e, v) => {
              const images = imageFiles(e.dataTransfer)
              if (images.length === 0) return false
              e.preventDefault()
              void addImages(images, v.posAtCoords({ x: e.clientX, y: e.clientY }) ?? undefined)
              return true
            },
            paste: (e, v) => {
              // a picture without text (a screenshot) becomes an asset; a copied web page brings text too
              const images = imageFiles(e.clipboardData)
              if (images.length > 0 && !e.clipboardData?.getData('text/plain')) {
                e.preventDefault()
                void addImages(images)
                return true
              }
              // nothing usable in the event (some webviews leave out the picture): ask the desktop shell
              const types = Array.from(e.clipboardData?.types ?? [])
              if (backend.clipboardImage && !types.some((t) => t === 'text/plain' || t === 'text/html')) {
                e.preventDefault()
                void backend.clipboardImage().then((b64) => {
                  if (b64) void addImages([pngFromBase64(b64)])
                })
                return true
              }
              const data = e.clipboardData?.getData('text/plain') ?? ''
              // a link copied from Settings → Unused images: `![](assets/...)` is written relative to this page
              const asset = /^!\[\]\((assets\/[^)\s]+)\)$/.exec(data.trim())
              if (asset) {
                e.preventDefault()
                const v0 = v
                void backend.call('asset_link', { page: pageName, path: asset[1] }).then((r) => {
                  if (view === v0) insertLinks(v0, [r.value])
                })
                return true
              }
              // an address, or a link copied from a page: written as a link (ED-27)
              if (!plain) {
                const sel = v.state.selection.main
                const line = v.state.doc.lineAt(sel.from)
                const r = smartPaste({
                  plain: data,
                  html: e.clipboardData?.getData('text/html') ?? '',
                  selected: v.state.sliceDoc(sel.from, sel.to),
                  before: v.state.sliceDoc(line.from, sel.from),
                  titles: app.pasteTitles,
                })
                if (r) {
                  e.preventDefault()
                  v.dispatch({
                    changes: { from: sel.from, to: sel.to, insert: r.insert },
                    selection: { anchor: sel.from + r.insert.length },
                    userEvent: 'input.paste',
                  })
                  if (r.fetchTitle) void addTitle(v, r.fetchTitle, sel.from)
                  return true
                }
              }
              if (plain || !/\n/.test(data.trim())) return false
              e.preventDefault()
              clearTimeout(idle)
              const sel = v.state.selection.main
              if (sel.from !== sel.to) v.dispatch({ changes: { from: sel.from, to: sel.to, insert: '' } })
              void onpaste(data)
              return true
            },
          }),
        ],
      }),
    })
    const len = view.state.doc.length
    const pos = cursor === 'end' ? len : cursor === 'start' ? 0 : Math.min(cursor, len)
    view.dispatch({ selection: { anchor: pos }, scrollIntoView: true })
    view.focus()
    return () => {
      clearTimeout(idle)
      view?.destroy()
      view = undefined
    }
  })

  /** Used by the parent to read the live text (e.g. on route changes). */
  /** Gives the editor the focus back (a move re-parents its DOM, which takes the focus away) and keeps the cursor in
   * view. */
  export function focus(): void {
    if (!view) return
    view.focus()
    view.dispatch({ effects: EditorView.scrollIntoView(view.state.selection.main.head, { y: 'nearest' }) })
  }
  // a command asks for the focus (see `editing.focusNonce`)
  $effect(() => {
    void editing.focusNonce
    untrack(() => focus())
  })
  export function getText(): string {
    return currentText()
  }
</script>

<div class="editor" bind:this={host}>
  {#if picker}
    <div
      class="picker-anchor"
      style:left="{picker.kind === 'file' ? 0 : picker.x}px"
      style:top="{picker.y}px"
    >
      {#if picker.kind === 'emoji'}
        <EmojiPicker onpick={pickEmoji} oncancel={cancelEmoji} />
      {:else if picker.kind === 'template'}
        <TemplatePicker onpick={pickTemplate} oncancel={closePicker} />
      {:else if picker.kind === 'file'}
        <FilePicker {pageName} oninsert={pickFile} oncancel={closePicker} />
      {:else}
        <DatePicker
          title={DATE_TITLES[picker.kind]}
          withTime={picker.kind !== 'link'}
          onpick={pickDate}
          oncancel={closePicker}
        />
      {/if}
    </div>
  {/if}
</div>

<style>
  .editor {
    width: 100%;
    position: relative;
  }
  .picker-anchor {
    position: absolute;
    z-index: 20;
  }
  .editor :global(.cm-editor) {
    outline: none;
    background: transparent;
    font: inherit;
  }
  /* the same font, size and line height as the rendered block (`.body` in Block.svelte), so the text
     does not move when editing starts; CodeMirror's own defaults are monospace with a 1.4 line height */
  .editor :global(.cm-scroller) {
    font-family: inherit;
    line-height: inherit;
  }
  .editor :global(.cm-content) {
    padding: 0;
    caret-color: var(--text);
    font-family: inherit;
    line-height: inherit;
  }
  .editor :global(.cm-line) {
    padding: 0;
  }
  .editor :global(.cm-wikilink) {
    color: var(--link);
  }
  .editor :global(.cm-tag) {
    color: var(--tag);
  }
  .editor :global(.cm-blockref) {
    color: var(--muted);
  }
  .editor :global(.cm-blockid) {
    color: var(--muted);
    opacity: 0.7;
  }
  .editor :global(.cm-hl) {
    background: var(--highlight);
  }
</style>
