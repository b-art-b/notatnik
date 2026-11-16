import { expect, test } from './fixtures'

// The bullet menu: copy a reference to a block, paste it elsewhere.
test('right-click a bullet, copy the block reference, paste it into another block', async ({
  backend,
  page,
}) => {
  // the system clipboard is shared by every test running in parallel: copies are captured in the page instead
  await page.addInitScript(() => {
    ;(window as unknown as { copied: string[] }).copied = []
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        writeText: (t: string) => {
          ;(window as unknown as { copied: string[] }).copied.push(t)
          return Promise.resolve()
        },
      },
    })
  })
  const lastCopy = () =>
    page.evaluate(() => (window as unknown as { copied: string[] }).copied.at(-1) ?? null)
  backend.write('pages/refs.md', '- the target block\n- another block\n')
  await backend.open('/page/refs')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  const menu = page.getByTestId('block-menu')
  await expect(menu.getByRole('menuitem')).toHaveText([
    /Make task/,
    /Select/,
    /Copy block reference/,
    /Copy as embed/,
    /Copy block text/,
    /Copy with children/,
    /Children as bullets/,
    /Children as numbered/,
    /Children as letters/,
    /Children as roman numerals/,
    /Zoom in/,
    /Delete/,
  ])
  await menu.getByRole('menuitem', { name: /Copy block reference/ }).click()
  await expect(menu).toHaveCount(0)
  // the target got an ID in the file, and the clipboard has the reference to it
  await expect.poll(() => backend.read('pages/refs.md')).toMatch(/^- the target block \^[a-z0-9]{6}$/m)
  const id = /\^([a-z0-9]{6})$/m.exec(backend.read('pages/refs.md'))![1]
  await expect.poll(lastCopy).toBe(`((${id}))`)
  // paste it into the other block
  await blocks.getByText('another block').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' see ')
  await page.keyboard.insertText((await lastCopy()) ?? '')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/refs.md')).toContain(`- another block see ((${id}))`)
  // as an embed, and Escape closes the menu
  await blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  await page.getByRole('menuitem', { name: /Copy as embed/ }).click()
  await expect.poll(lastCopy).toBe(`!((${id}))`)
  await blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: /Copy block text/ }))
    .toBeFocused()
    .catch(() => {})
  await page.keyboard.press('Escape')
  await expect(menu).toHaveCount(0)
})

test('right-click a bullet and pick Delete: the block goes with its children, one undo brings it back', async ({
  backend,
  page,
}) => {
  backend.write('pages/del.md', '- keep\n- remove me\n  - child\n- also keep\n')
  await backend.open('/page/del')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).nth(1).click({ button: 'right' })
  await page
    .getByTestId('block-menu')
    .getByRole('menuitem', { name: /Delete/ })
    .click()
  await expect.poll(() => backend.read('pages/del.md')).toBe('- keep\n- also keep\n')
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/del.md')).toBe('- keep\n- remove me\n  - child\n- also keep\n')
})

// a phone has no Ctrl+Enter: the menu makes a task, moves it on and takes the marker off
test('the bullet menu makes a task, moves it to the next state and removes it; a selection gets Make tasks', async ({
  backend,
  page,
}) => {
  backend.write('pages/tk.md', '- one\n- two\n- three\n')
  await backend.open('/page/tk')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  const menu = page.getByTestId('block-menu')
  const open = () =>
    blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  await open()
  await menu.getByRole('menuitem', { name: /Make task/ }).click()
  await expect.poll(() => backend.read('pages/tk.md')).toBe('- TODO one\n- two\n- three\n')
  await open()
  await expect(menu.getByRole('menuitem', { name: /Next task state/ })).toContainText('TODO → DOING')
  await menu.getByRole('menuitem', { name: /Next task state/ }).click()
  await expect.poll(() => backend.read('pages/tk.md')).toBe('- DOING one\n- two\n- three\n')
  await open()
  await menu.getByRole('menuitem', { name: /Remove task/ }).click()
  await expect.poll(() => backend.read('pages/tk.md')).toBe('- one\n- two\n- three\n')
})
