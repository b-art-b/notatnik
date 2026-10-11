// Opens a page and flashes one block in it (used by search results, the agenda, ...).
import { navigate } from './stores/app.svelte'

/** Child-index path of the block to flash; `null` just opens the page. */
export function jumpToBlock(page: string, path: number[] | null): void {
  try {
    sessionStorage.setItem('notatnik-flash', JSON.stringify({ page, path }))
  } catch {
    /* without storage the page still opens, only the flash is skipped */
  }
  navigate({ kind: 'page', name: page, zoom: [] })
  // the page renders asynchronously: tell it to flash once it had time to
  setTimeout(() => window.dispatchEvent(new CustomEvent('notatnik-flash')), 150)
}

/** Click handler for a normal link that should flash the block: plain clicks jump, modified clicks keep the browser's behaviour. */
export function jumpOnClick(e: MouseEvent, page: string, path: number[] | null): void {
  if (e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return
  e.preventDefault()
  jumpToBlock(page, path)
}
