import { expect, test } from './fixtures'

// WA-1: the clip dialog on a tablet (emulated touch screen, coarse pointer): its controls are 44 px high.
test.use({ viewport: { width: 1024, height: 768 }, hasTouch: true, isMobile: true })

test('the clip dialog has 44 px targets for a finger', async ({ backend, page }) => {
  backend.write('pages/notes.md', '- start\n')
  await backend.open('/page/notes')
  await page.evaluate(() => (window.location.hash = '#/page/notes'))
  await page.keyboard.press('Control+k')
  await page.getByLabel('Command or page').fill('clip a web')
  await page.keyboard.press('Enter')
  await page.getByTestId('clip-address').fill('https://example.org/a')
  for (const el of [
    page.getByTestId('clip-address'),
    page.getByTestId('clip-go'),
    page.getByRole('button', { name: 'Close' }),
  ]) {
    const box = (await el.boundingBox())!
    expect(box.height).toBeGreaterThanOrEqual(44)
  }
})
