import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { expect } from './fixtures'
import { makePdf, REPORT } from './pdf-fixture'
import { test } from './fixtures'

// LV-2: the arrow after a link on a tablet (emulated touch screen, coarse pointer) is always shown and 44 px.
test.use({ viewport: { width: 1024, height: 768 }, hasTouch: true, isMobile: true, localFiles: true })

test('the arrow after a PDF link is shown and 44 px for a finger', async ({ backend, page }) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'notatnik-local-'))
  const pdf = path.join(dir, 'Report.pdf')
  fs.writeFileSync(pdf, makePdf(REPORT))
  backend.write('pages/reading.md', `- [the report](file://${pdf})\n`)
  await backend.open('/page/reading')
  const arrow = page.getByRole('tree', { name: 'Blocks' }).getByRole('button', { name: /in the system/ })
  await expect(arrow).toHaveCSS('opacity', '1')
  const box = (await arrow.boundingBox())!
  expect(box.height).toBeGreaterThanOrEqual(44)
  expect(box.width).toBeGreaterThanOrEqual(44)
})
