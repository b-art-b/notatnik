import { expect, test } from './fixtures'

// PL-5: the app on an iPad. A touch screen has no hover and no right button, so what hover or a right click gives on
// a desktop has to be on screen or one press away. The browser is emulated as a tablet (touch, coarse pointer, no hover).
test.use({ viewport: { width: 1024, height: 768 }, hasTouch: true, isMobile: true })

const TREE = '- parent\n  - child\n'

test('the expand arrow of a block is on screen without hover', async ({ backend, page }) => {
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  const arrow = page.getByRole('tree', { name: 'Blocks' }).locator('.arrow').first()
  await expect(arrow).toBeVisible()
  // opacity 0 counts as visible for Playwright: assert the computed style
  await expect(arrow).toHaveCSS('opacity', '1')
  // and it is a target a finger can hit
  const box = (await arrow.boundingBox())!
  expect(box.width).toBeGreaterThanOrEqual(24)
})

test('pressing and holding a bullet opens the block menu and does not zoom into the block', async ({
  backend,
  page,
}) => {
  backend.write('pages/t2.md', TREE)
  await backend.open('/page/t2')
  const bullet = page
    .getByRole('tree', { name: 'Blocks' })
    .getByRole('button', { name: 'Zoom into block' })
    .first()
  const before = page.url()
  const box = (await bullet.boundingBox())!
  const at = { clientX: box.x + box.width / 2, clientY: box.y + box.height / 2, pointerType: 'touch' }
  await bullet.dispatchEvent('pointerdown', { ...at, isPrimary: true, pointerId: 7 })
  await expect(page.getByRole('menu')).toBeVisible({ timeout: 3000 })
  // the finger lifts: the click that follows does not open the block
  await bullet.dispatchEvent('pointerup', { ...at, isPrimary: true, pointerId: 7 })
  await bullet.dispatchEvent('click', { clientX: at.clientX, clientY: at.clientY })
  expect(page.url()).toBe(before)
  await expect(page.getByRole('menu')).toBeVisible()
})

test('a short tap on a bullet still zooms into the block', async ({ backend, page }) => {
  backend.write('pages/t3.md', TREE)
  await backend.open('/page/t3')
  const before = page.url()
  await page
    .getByRole('tree', { name: 'Blocks' })
    .getByRole('button', { name: 'Zoom into block' })
    .first()
    .tap()
  await expect.poll(() => page.url()).not.toBe(before)
  await expect(page.getByRole('menu')).toHaveCount(0)
})
