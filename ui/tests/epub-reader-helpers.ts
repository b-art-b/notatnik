import { expect, type Frame, type Page } from '@playwright/test'

// Helpers for the tests that read a book in the EPUB reader (the chapters are in frames).

export const chapterFrames = (page: Page): Frame[] => page.frames().filter((f) => f !== page.mainFrame())

export async function bookText(page: Page): Promise<string> {
  const texts = await Promise.all(
    chapterFrames(page).map((f) =>
      f
        .locator('body')
        .innerText()
        .catch(() => ''),
    ),
  )
  return texts.join('\n')
}
export const reads = (page: Page, text: string) =>
  expect.poll(() => bookText(page), { timeout: 15_000 }).toContain(text)

/** Selects the text `text` in the chapter that holds it (as a drag would). */
export async function select(page: Page, text: string) {
  for (const f of chapterFrames(page)) {
    const done = await f.evaluate((text) => {
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const i = (n as Text).data.indexOf(text)
        if (i < 0) continue
        const r = document.createRange()
        r.setStart(n, i)
        r.setEnd(n, i + text.length)
        const sel = window.getSelection()!
        sel.removeAllRanges()
        sel.addRange(r)
        return true
      }
      return false
    }, text)
    if (done) return
  }
  throw new Error(`text not found in any chapter: ${text}`)
}

export async function highlight(page: Page, text: string) {
  await select(page, text)
  await expect(page.getByTestId('pdf-selbar')).toBeVisible()
  await page.getByTestId('pdf-color-yellow').click()
}
