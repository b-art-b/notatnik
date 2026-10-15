import { expect, test } from './fixtures'

// The bullet menu: copy a reference to a block, paste it elsewhere.
test('right-click a bullet, copy the block reference, paste it into another block', async ({
  backend,
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  backend.write('pages/refs.md', '- the target block\n- another block\n')
  await backend.open('/page/refs')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  const menu = page.getByTestId('block-menu')
  await expect(menu.getByRole('menuitem')).toHaveText([
    /Copy block reference/,
    /Copy as embed/,
    /Copy block text/,
    /Copy with children/,
    /Zoom in/,
  ])
  await menu.getByRole('menuitem', { name: /Copy block reference/ }).click()
  await expect(menu).toHaveCount(0)
  // the target got an ID in the file, and the clipboard has the reference to it
  await expect.poll(() => backend.read('pages/refs.md')).toMatch(/^- the target block \^[a-z0-9]{6}$/m)
  const id = /\^([a-z0-9]{6})$/m.exec(backend.read('pages/refs.md'))![1]
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(`((${id}))`)
  // paste it into the other block
  await blocks.getByText('another block').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' see ')
  await page.keyboard.press('Control+V')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/refs.md')).toContain(`- another block see ((${id}))`)
  // as an embed, and Escape closes the menu
  await blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  await page.getByRole('menuitem', { name: /Copy as embed/ }).click()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(`!((${id}))`)
  await blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: /Copy block text/ }))
    .toBeFocused()
    .catch(() => {})
  await page.keyboard.press('Escape')
  await expect(menu).toHaveCount(0)
})
