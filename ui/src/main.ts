import './assets/fonts/inter.css'
import { mount } from 'svelte'
import App from './App.svelte'
import { backend, CommandError } from './lib/api'

mount(App, { target: document.getElementById('app')! })

// A file dropped anywhere but on an editor would make the webview open it and leave the app (ST-8).
for (const type of ['dragover', 'drop']) {
  window.addEventListener(type, (e) => {
    if ((e as DragEvent).dataTransfer?.types.includes('Files')) e.preventDefault()
  })
}

// The desktop app has no visible console: an error nobody caught goes to the terminal log of the app as well.
// A failed command was logged by the backend already.
function toTerminal(message: string) {
  backend.call('log_client', { level: 'error', message }).catch(() => {})
}
window.addEventListener('error', (e) => {
  // the browser's own note that a resize was handled in the next frame; layout code (the EPUB reader) triggers it, harmless
  if (e.message.startsWith('ResizeObserver loop')) return
  // foliate-js (the EPUB reader) resizes a chapter's frame once more while the frame is being removed, WebKit then has no
  // document for it; nothing is lost, the reader is closing
  if (e.filename.includes('/paginator') && e.message.includes('documentElement')) return
  toTerminal(`${e.message} (${e.filename}:${e.lineno})`)
})
window.addEventListener('unhandledrejection', (e) => {
  if (e.reason instanceof CommandError) return
  toTerminal(
    `unhandled rejection: ${e.reason instanceof Error ? (e.reason.stack ?? e.reason.message) : String(e.reason)}`,
  )
})
