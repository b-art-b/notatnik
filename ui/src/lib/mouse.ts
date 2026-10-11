// Firefox-style navigation with the mouse and the keyboard, installed once for the whole window.
import { app } from './stores/app.svelte'
import { parseRoute } from './route'

/** The page a link points to, if it is an in-app page link (`#/page/<name>`). */
function pageLinkTarget(el: EventTarget | null): string | null {
  const a = (el as HTMLElement | null)?.closest?.('a[href^="#/page/"]')
  if (!a) return null
  const r = parseRoute(a.getAttribute('href') ?? '')
  return r.kind === 'page' ? r.name : null
}

function openInSidebar(name: string) {
  if (!app.rightPanels.includes(name)) app.rightPanels.push(name)
}

/**
 * - mouse back / forward buttons (4 and 5) go back / forward in the page history
 * - Alt+← / Alt+→ do the same when no text field has the focus (there they move the cursor)
 * - middle click or Ctrl+click on a page link opens it in the right sidebar ("new tab")
 * Returns a function that removes the listeners.
 */
export function installNavigation(): () => void {
  // act on mouseup; swallow mousedown too so the browser's own handling doesn't run as well
  const swallow = (e: MouseEvent) => {
    if (e.button === 3 || e.button === 4) e.preventDefault()
    // middle button: no autoscroll cursor when it's over a page link
    if (e.button === 1 && pageLinkTarget(e.target)) e.preventDefault()
  }
  const up = (e: MouseEvent) => {
    if (e.button === 3) {
      e.preventDefault()
      history.back()
    } else if (e.button === 4) {
      e.preventDefault()
      history.forward()
    }
  }
  const aux = (e: MouseEvent) => {
    if (e.button !== 1) return
    const name = pageLinkTarget(e.target)
    if (name === null) return
    e.preventDefault()
    openInSidebar(name)
  }
  const click = (e: MouseEvent) => {
    if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.button !== 0) return
    const name = pageLinkTarget(e.target)
    if (name === null) return
    e.preventDefault()
    e.stopPropagation()
    openInSidebar(name)
  }
  const key = (e: KeyboardEvent) => {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return
    if ((e.target as HTMLElement)?.closest?.('input, textarea, select, .cm-editor')) return
    e.preventDefault()
    if (e.key === 'ArrowLeft') history.back()
    else history.forward()
  }
  window.addEventListener('mousedown', swallow, true)
  window.addEventListener('mouseup', up, true)
  window.addEventListener('auxclick', aux, true)
  window.addEventListener('click', click, true)
  window.addEventListener('keydown', key)
  return () => {
    window.removeEventListener('mousedown', swallow, true)
    window.removeEventListener('mouseup', up, true)
    window.removeEventListener('auxclick', aux, true)
    window.removeEventListener('click', click, true)
    window.removeEventListener('keydown', key)
  }
}
