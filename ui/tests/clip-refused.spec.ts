import { expect, test } from './fixtures'
import { pageServer } from './web-fixture'

// WA-7: without --allow-local-fetch (the way the app runs) an address on this computer is refused, and nothing is written.
test('an address on this computer is refused with a message', async ({ backend, page }) => {
  const web = await pageServer()
  try {
    backend.write('pages/notes.md', '- start\n')
    await backend.open('/page/notes')
    await page.keyboard.press('Control+k')
    await page.getByLabel('Command or page').fill('clip a web')
    await page.keyboard.press('Enter')
    await page.getByTestId('clip-address').fill(`${web.base}/article`)
    await page.getByTestId('clip-go').click()
    await expect(page.getByTestId('clip-error')).toBeVisible()
    await expect(page.getByTestId('clip-error')).toContainText(/address|network|public/i)
    expect(backend.exists('assets/web')).toBe(false)
  } finally {
    await web.close()
  }
})
