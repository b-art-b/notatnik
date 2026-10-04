<script lang="ts">
  // One CodeMirror 6 instance, mounted on the focused block only (ED-2). It edits the block's raw
  // Markdown. Svelte idiom: `onMount` creates the view, and the returned function destroys it.
  import { onMount } from 'svelte'
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
  import { markdown } from '@codemirror/lang-markdown'
  import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language'
  import type { Cursor } from '../tree'
  import type { EditorAction } from '../commands/exec'
  import { wikiCompletions } from '../completion'

  interface Props {
    text: string
    cursor: Cursor
    /** Current page name (completions rank its namespace first). */
    pageName: string
    /** Called ~500 ms after the last keystroke, and when the block loses focus. */
    onchange: (text: string) => void
    onaction: (a: EditorAction) => Promise<boolean>
    onundo: (kind: 'undo' | 'redo') => void
    onpaste: (text: string) => Promise<boolean>
    onblur: (text: string) => void
  }
  let { text, cursor, pageName, onchange, onaction, onundo, onpaste, onblur }: Props = $props()

  let host: HTMLDivElement
  let view: EditorView | undefined
  let idle: ReturnType<typeof setTimeout> | undefined
  let lastSent = text

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
      lastSent = t
      onchange(t)
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
            const { head } = lineInfo(v)
            if (v.state.selection.main.from !== v.state.selection.main.to) return false
            // run the split; if it declines (cursor not on the first line, in a fence) insert a newline
            clearTimeout(idle)
            void onaction({ kind: 'enter', text: v.state.doc.toString(), cursor: head }).then((done) => {
              if (!done) insertNewline(v)
            })
            return true
          },
        },
        { key: 'Shift-Enter', run: insertNewline },
        { key: 'Tab', run: (v) => act(v, (t) => ({ kind: 'indent', text: t })) },
        { key: 'Shift-Tab', run: (v) => act(v, (t) => ({ kind: 'outdent', text: t })) },
        { key: 'Alt-ArrowUp', run: (v) => act(v, (t) => ({ kind: 'move-up', text: t })) },
        { key: 'Alt-ArrowDown', run: (v) => act(v, (t) => ({ kind: 'move-down', text: t })) },
        {
          key: 'ArrowUp',
          run: (v) => {
            const i = lineInfo(v)
            if (!i.first) return false
            clearTimeout(idle)
            void onaction({ kind: 'nav-up', text: v.state.doc.toString(), column: i.column })
            return true
          },
        },
        {
          key: 'ArrowDown',
          run: (v) => {
            const i = lineInfo(v)
            if (!i.last) return false
            clearTimeout(idle)
            void onaction({ kind: 'nav-down', text: v.state.doc.toString(), column: i.column })
            return true
          },
        },
        {
          key: 'Backspace',
          run: (v) => {
            const s = v.state.selection.main
            if (s.from !== 0 || s.to !== 0) return false
            clearTimeout(idle)
            void onaction({ kind: 'merge-prev', text: v.state.doc.toString() })
            return true
          },
        },
        {
          key: 'Delete',
          run: (v) => {
            const s = v.state.selection.main
            if (s.from !== v.state.doc.length || s.to !== v.state.doc.length) return false
            clearTimeout(idle)
            void onaction({ kind: 'merge-next', text: v.state.doc.toString() })
            return true
          },
        },
        { key: 'Escape', run: (v) => act(v, (t) => ({ kind: 'escape', text: t })) },
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
          wikiCompletions(() => pageName),
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
                if (view && !view.hasFocus && !document.activeElement?.closest('.cm-tooltip')) {
                  clearTimeout(idle)
                  const t = currentText()
                  lastSent = t
                  onblur(t)
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
  export function getText(): string {
    return currentText()
  }
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    width: 100%;
  }
  .editor :global(.cm-editor) {
    outline: none;
    background: transparent;
    font: inherit;
  }
  .editor :global(.cm-content) {
    padding: 0;
    caret-color: var(--text);
    font-family: var(--font-mono);
    font-size: 0.93em;
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
