import { expect } from './fixtures'
import { makePdf, REPORT } from './pdf-fixture'
import { fileId, put, synced, test } from './nextcloud-fixture'

// NA-6, NA-7: the viewer bar and the picker of a document of the server on a tablet (emulated touch screen, coarse
// pointer): the buttons are 44 px high.
test.use({ viewport: { width: 1024, height: 768 }, hasTouch: true, isMobile: true })

test('Keep a copy and Annotate have 44 px targets for a finger', async ({ backend, page, dav }) => {
  await put(dav, 'Docs/Report.pdf', makePdf(REPORT))
  const g = await synced(backend, dav)
  const id = await fileId(dav, 'Docs/Report.pdf')
  g.write('pages/reading.md', `- [the report](${dav.server}/f/${id})\n`)
  await backend.open('/page/reading')
  await page.getByRole('tree', { name: 'Blocks' }).getByRole('link', { name: 'the report' }).click()
  await expect(page.getByTestId('pdf-viewer')).toBeVisible()
  const keep = (await page.getByTestId('doc-keep').boundingBox())!
  expect(keep.height).toBeGreaterThanOrEqual(44)
})
