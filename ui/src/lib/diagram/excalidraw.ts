// Excalidraw as a lazy, offline island (DG-5, DG-6, X2). Excalidraw is a React component; nothing here is React code
// we write: `mountDiagram` loads React and Excalidraw the first time a diagram is opened (a separate chunk, so a note
// without a diagram never pays for it), mounts the editor into a <div> and gives back plain functions.
//
// A diagram is stored as an SVG with its scene inside (`exportEmbedScene`), so the file is an image anywhere and opens
// again as separate, editable shapes (DG-2).

import type { ExcalidrawImperativeAPI } from '@excalidraw/excalidraw/types'
import type { Root } from 'react-dom/client'

/** What a mounted editor offers the overlay around it. */
export interface DiagramHandle {
  /** A stored file was given but holds no Excalidraw scene (another program rewrote the SVG): the editor is blank. */
  readonly noScene: boolean
  /** The diagram as an SVG with its scene embedded, on a white background. */
  getSvg(): Promise<string>
  /** Nothing is drawn (the SVG of an empty diagram is not stored). */
  isEmpty(): boolean
  /** The picture changed since it was opened (or since `markSaved`). */
  changed(): boolean
  /** Remember the current picture as the saved one. */
  markSaved(): void
  /** Take the editor out of the page. */
  unmount(): void
}

export interface MountOptions {
  /** The SVG of a stored diagram, to edit it again; none: a new, empty diagram. */
  svg?: string
  /** The app's theme; the stored picture is on white whatever this is. */
  theme: 'light' | 'dark'
  /** Called when the scene changes (a draft saver listens to this). */
  onChange?: () => void
}

/** The fonts are copied to `/excalidraw/fonts/` by the build (`excalidrawFiles` in vite.config.ts). */
const ASSET_PATH = '/excalidraw/'

/** Hides the entries that talk to excalidraw.com and the ones a note does not need (a library browser, a link to the
 * hosted app, the community links). The editor itself and the local libraries stay. */
const HIDE = `
.excalidraw .library-menu-browse-button,
.excalidraw .layer-ui__wrapper__footer-right,
.excalidraw .welcome-screen-center,
.excalidraw .help-icon { display: none !important; }
`

let styled = false
function addStyle(): void {
  if (styled) return
  styled = true
  const el = document.createElement('style')
  el.dataset.excalidraw = 'hide'
  el.textContent = HIDE
  document.head.appendChild(el)
}

export async function mountDiagram(container: HTMLElement, opts: MountOptions): Promise<DiagramHandle> {
  ;(window as unknown as { EXCALIDRAW_ASSET_PATH?: string }).EXCALIDRAW_ASSET_PATH = ASSET_PATH
  const [React, { createRoot }, ex] = await Promise.all([
    import('react'),
    import('react-dom/client'),
    import('@excalidraw/excalidraw'),
    import('@excalidraw/excalidraw/index.css'),
  ])
  addStyle()

  // the stored scene, when there is one: `loadFromBlob` reads the scene back out of the SVG
  let initial: Awaited<ReturnType<typeof ex.loadFromBlob>> | null = null
  let noScene = false
  if (opts.svg) {
    try {
      initial = await ex.loadFromBlob(new Blob([opts.svg], { type: 'image/svg+xml' }), null, null)
    } catch {
      noScene = true
    }
    // an SVG without the embedded scene: the picture shows in a note, but there are no shapes to edit
    if (initial && initial.elements.length === 0 && !opts.svg.includes('application/vnd.excalidraw+json')) {
      noScene = true
      initial = null
    }
  }

  let api: ExcalidrawImperativeAPI | null = null
  const sceneKey = (): string =>
    api ? ex.serializeAsJSON(api.getSceneElements(), api.getAppState(), api.getFiles(), 'local') : ''
  let saved = ''

  const element = React.createElement(ex.Excalidraw, {
    excalidrawAPI: (a: ExcalidrawImperativeAPI) => {
      api = a
      // the opened picture is the saved one; set after the first paint so the editor's own set-up is not a change
      setTimeout(() => {
        saved = sceneKey()
      }, 0)
    },
    initialData: initial
      ? {
          elements: initial.elements,
          appState: { ...initial.appState, collaborators: new Map() },
          files: initial.files,
        }
      : null,
    theme: opts.theme,
    langCode: 'en',
    // no hosted features: no collaboration, no sharing, no loading or saving of files on their own
    UIOptions: {
      canvasActions: {
        loadScene: false,
        saveToActiveFile: false,
        export: false,
        saveAsImage: false,
        toggleTheme: false,
      },
    },
    onChange: () => opts.onChange?.(),
  })
  const root: Root = createRoot(container)
  root.render(element)

  const live = (): ExcalidrawImperativeAPI => {
    if (!api) throw new Error('the diagram editor is not ready')
    return api
  }
  return {
    noScene,
    async getSvg() {
      const a = live()
      const svg = await ex.exportToSvg({
        elements: a.getSceneElements(),
        appState: {
          ...a.getAppState(),
          exportEmbedScene: true,
          exportBackground: true,
          viewBackgroundColor: '#ffffff',
          exportWithDarkMode: false,
        },
        files: a.getFiles(),
        exportPadding: 12,
      })
      return new XMLSerializer().serializeToString(svg)
    },
    isEmpty: () => (api ? api.getSceneElements().length === 0 : true),
    changed: () => (api ? sceneKey() !== saved : false),
    markSaved() {
      saved = sceneKey()
    },
    unmount() {
      root.unmount()
    },
  }
}
