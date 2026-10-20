import { expect, test } from './fixtures'

// The block being edited is marked: its editor looks like the rendered text on purpose, so a bullet that holds
// only a link looked the same in both states.
test('the block being edited has an outline and a tint, the others none', async ({ backend, page }) => {
  backend.write('pages/Tools.md', '- https://github.com/a/b - a tool\n- https://github.com/c/d - another\n')
  await backend.open('/page/Tools')
  const tree = page.getByRole('tree', { name: 'Blocks' })
  const body = (n: number) => tree.locator('.body').nth(n)
  await expect(body(0)).toHaveCSS('outline-style', 'none')
  await tree.getByText('a tool').click({ position: { x: 4, y: 4 } })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  const editing = page.locator('.body.editing')
  await expect(editing).toHaveCount(1)
  await expect(editing).toHaveCSS('outline-style', 'solid')
  await expect(editing).not.toHaveCSS('background-color', 'rgba(0, 0, 0, 0)')
  await expect(body(1)).toHaveCSS('outline-style', 'none')
  await page.keyboard.press('Escape')
  await expect(page.locator('.body.editing')).toHaveCount(0)
})
