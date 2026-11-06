import { expect, test } from './fixtures'
import { makePdf, REPORT } from './pdf-fixture'
import fs from 'node:fs'
import path from 'node:path'

// DD-1: Dock and Full screen on a tablet in landscape (emulated touch screen, coarse pointer): 44 px, and it works.
test.use({ viewport: { width: 1100, height: 800 }, hasTouch: true, isMobile: true })

test('the Dock button is 44 px and docks the PDF beside the notes', async ({ backend, page }) => {
  fs.mkdirSync(path.join(backend.dir, 'assets'), { recursive: true })
  fs.writeFileSync(path.join(backend.dir, 'assets/report.pdf'), makePdf(REPORT))
  backend.write('pages/reading.md', '- Read [the report](../assets/report.pdf)\n')
  await backend.open('/page/reading')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'the report' }).click()
  const dock = page.getByTestId('doc-dock')
  expect((await dock.boundingBox())!.height).toBeGreaterThanOrEqual(44)
  await dock.tap()
  await expect(page.getByTestId('pdf-viewer')).toHaveAttribute('role', 'region')
  const back = page.getByTestId('doc-dock')
  await expect(back).toHaveText('Full screen')
  expect((await back.boundingBox())!.height).toBeGreaterThanOrEqual(44)
})
