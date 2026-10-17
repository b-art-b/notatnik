import { expect, test } from './fixtures'
// Shift+Tab as WebKitGTK reports it: key `ISO_Left_Tab`
test('ISO_Left_Tab outdents in the editor', async ({ backend, page }) => {
  backend.write('pages/t.md', '- a\n  - a1\n  - a2\n- b\n')
  await backend.open('/page/t')
  await page.waitForTimeout(500)
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByText('a1', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.evaluate(() => {
    const ed = document.querySelector('[data-testid="block-editor"]')!
    ed.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'ISO_Left_Tab',
        code: 'Tab',
        shiftKey: true,
        keyCode: 9,
        bubbles: true,
        cancelable: true,
      }),
    )
  })
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n- a1\n  - a2\n- b\n')
})

test('ISO_Left_Tab outdents a selection', async ({ backend, page }) => {
  backend.write('pages/t.md', '- a\n  - a1\n  - a2\n- b\n')
  await backend.open('/page/t')
  await page.waitForTimeout(500)
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByText('a1', { exact: true }).click()
  await blocks.getByText('b', { exact: true }).click({ modifiers: ['Shift'] })
  await expect(blocks.getByRole('treeitem', { selected: true })).toHaveCount(3)
  await page.evaluate(() => {
    ;(document.activeElement ?? document.body).dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'ISO_Left_Tab',
        code: 'Tab',
        shiftKey: true,
        keyCode: 9,
        bubbles: true,
        cancelable: true,
      }),
    )
  })
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n- a1\n- a2\n- b\n')
})
