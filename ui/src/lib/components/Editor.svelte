<script lang="ts">
  // One CodeMirror 6 instance, mounted on the focused block only (ED-2). It edits the block's raw
  // Markdown. Svelte idiom: `onMount` creates the view, and the returned function destroys it.
  import { onMount, untrack } from 'svelte'
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
    insertNewline,
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
  import { dateChanges, DATE_TITLES, refreshTemplateNames, type DateKind } from '../commands/slash'
  import DatePicker from './DatePicker.svelte'
  import EmojiPicker from './EmojiPicker.svelte'

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
  }
  let { text, cursor, pageName, onchange, onaction, onundo, onpaste, onblur }: Props = $props()

  let host: HTMLDivElement
  let view: EditorView | undefined
  let idle: ReturnType<typeof setTimeout> | undefined
  // the text the editor starts with; updated as changes are sent (not reactive on purpose)
  let lastSent = untrack(() => text)
  /** The open date picker (from /date, /scheduled, /deadline), positioned under the cursor. */
  let picker = $state<{ kind: DateKind | 'emoji'; at: number; x: number; y: number } | null>(null)

  function openPicker(kind: DateKind | 'emoji', at: number) {
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
    if (!picker || !view || picker.kind === 'emoji') return
    const { kind, at } = picker
    view.dispatch({ changes: dateChanges(view.state.doc.toString(), at, kind, iso) })
    closePicker()
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

  function keys(): Extension {
    return Prec.highest(
      keymap.of([
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
          markdown(),
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
            paste: (e, v) => {
              const data = e.clipboardData?.getData('text/plain') ?? ''
              if (!/\n/.test(data.trim())) return false
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
  /** Gives the editor the focus back (a move re-parents its DOM, which takes the focus away). */
  export function focus(): void {
    view?.focus()
  }
  export function getText(): string {
    return currentText()
  }
</script>

<div class="editor" bind:this={host}>
  {#if picker}
    <div class="picker-anchor" style:left="{picker.x}px" style:top="{picker.y}px">
      {#if picker.kind === 'emoji'}
        <EmojiPicker onpick={pickEmoji} oncancel={cancelEmoji} />
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
