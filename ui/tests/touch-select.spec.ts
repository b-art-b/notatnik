import { expect, test } from './fixtures'

// A phone has no Shift, Ctrl or right button: "Select" in the bullet's menu starts a selection, a tap on another block
// adds it, and the bar at the bottom makes the selected blocks tasks.
test.use({ hasTouch: true, isMobile: true })

test('on a touch screen: Select in the menu, tap more blocks, Tasks in the bar', async ({
  backend,
  page,
}) => {
  backend.write('pages/ts.md', '- one\n- two\n- three\n- four\n')
  await backend.open('/page/ts')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  const bullet = (n: number) => blocks.getByRole('button', { name: 'Zoom into block' }).nth(n)
  await bullet(0).click({ button: 'right' })
  await page
    .getByTestId('block-menu')
    .getByRole('menuitem', { name: /Select/ })
    .click()
  const bar = page.getByTestId('touch-selection-bar')
  await expect(bar).toContainText('1 selected')
  await blocks.getByText('three', { exact: true }).tap()
  await expect(bar).toContainText('2 selected')
  // a tap on a selected block takes it out again
  await blocks.getByText('three', { exact: true }).tap()
  await expect(bar).toContainText('1 selected')
  await blocks.getByText('two', { exact: true }).tap()
  await blocks.getByText('four', { exact: true }).tap()
  await expect(bar).toContainText('3 selected')
  await bar.getByRole('button', { name: 'Tasks' }).click()
  await expect.poll(() => backend.read('pages/ts.md')).toBe('- TODO one\n- TODO two\n- three\n- TODO four\n')
  await bar.getByRole('button', { name: 'Done' }).click()
  await expect(bar).toHaveCount(0)
})

test('on a touch screen the bar moves, indents and outdents the selected blocks', async ({
  backend,
  page,
}) => {
  backend.write('pages/tm.md', '- a\n- b\n- c\n')
  await backend.open('/page/tm')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).nth(1).click({ button: 'right' })
  await page
    .getByTestId('block-menu')
    .getByRole('menuitem', { name: /Select/ })
    .click()
  const bar = page.getByTestId('touch-selection-bar')
  await bar.getByRole('button', { name: 'Move up' }).click()
  await expect.poll(() => backend.read('pages/tm.md')).toBe('- b\n- a\n- c\n')
  await bar.getByRole('button', { name: 'Move down' }).click()
  await expect.poll(() => backend.read('pages/tm.md')).toBe('- a\n- b\n- c\n')
  await bar.getByRole('button', { name: 'Indent' }).click()
  await expect.poll(() => backend.read('pages/tm.md')).toBe('- a\n  - b\n- c\n')
  await bar.getByRole('button', { name: 'Outdent' }).click()
  await expect.poll(() => backend.read('pages/tm.md')).toBe('- a\n- b\n- c\n')
})
