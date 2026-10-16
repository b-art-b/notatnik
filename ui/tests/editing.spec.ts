import { expect, test } from './fixtures'

test('typing into a block and pressing Esc writes the file', async ({ backend, page }) => {
  await backend.open('/page/inbox')
  await page.getByText('Call the dentist').click()
  const editor = page.getByTestId('block-editor')
  await expect(editor).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' tomorrow')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/inbox.md')).toContain('- Call the dentist #health tomorrow')
  // nothing else changed
  expect(backend.read('pages/inbox.md')).toContain('  - TODO Renew passport\n    deadline:: 2026-10-20')
})

test('Enter splits, Tab indents, Backspace merges, undo restores bytes', async ({ backend, page }) => {
  await backend.open('/page/inbox')
  const original = backend.read('pages/inbox.md')
  await page.getByText('Call the dentist').click()
  await page.keyboard.press('End')
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('block-editor')).toHaveText('')
  await page.keyboard.type('new sibling')
  await page.keyboard.press('Tab')
  await expect.poll(() => backend.read('pages/inbox.md')).toContain('    - new sibling')
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('block-editor')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await page.waitForTimeout(300) // let the UI settle: a human can't press keys this fast
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/inbox.md')).not.toContain('    - new sibling')
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/inbox.md')).toBe(original)
})

test('first content on an empty page creates the file; leaving empty creates none', async ({
  backend,
  page,
}) => {
  await backend.open('/page/brand%2Fnew')
  await page.getByText('Click here to start writing').click()
  await page.keyboard.press('Escape')
  expect(backend.exists('pages/brand/new.md')).toBe(false)
  await page.getByText('Click here to start writing').click()
  await page.keyboard.type('hello')
  await expect.poll(() => backend.exists('pages/brand/new.md')).toBe(true)
  await expect.poll(() => backend.read('pages/brand/new.md')).toBe('- hello\n')
})

test('clicking in the middle of a word puts the cursor there', async ({ backend, page }) => {
  backend.write('pages/click.md', '- hello wonderful world\n')
  await backend.open('/page/click')
  await expect(page.getByText('hello wonderful world')).toBeVisible()
  // click in the middle of "wonderful" (position taken from the real text layout)
  const pt = await page.evaluate(() => {
    const p = document.querySelector('.nodes p')!
    const w = document.createTreeWalker(p, NodeFilter.SHOW_TEXT)
    let node = w.nextNode()!
    while (node && !node.textContent?.trim()) node = w.nextNode()!
    const r = document.createRange()
    r.setStart(node, 8)
    r.setEnd(node, 9)
    const b = r.getBoundingClientRect()
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 }
  })
  await page.mouse.click(pt.x, pt.y)
  await expect(page.getByTestId('block-editor')).toBeFocused()
  const pos = await page.evaluate(() => {
    const s = window.getSelection()
    return s?.anchorOffset ?? -1
  })
  expect(pos).toBeGreaterThanOrEqual(7)
  expect(pos).toBeLessThanOrEqual(10)
})

test('starting to edit a block does not move its text', async ({ backend, page }) => {
  backend.write('pages/t.md', '- plain words in a block with a [[link]] and #tag\n')
  await backend.open('/page/t')
  const body = page.getByRole('tree', { name: 'Blocks' }).locator('.body').first()
  const before = await body.boundingBox()
  await body.click()
  await expect(page.locator('.cm-content')).toBeVisible()
  const line = await page.locator('.cm-line').first().boundingBox()
  expect(Math.abs(line!.y - before!.y)).toBeLessThan(1.5)
  expect(Math.abs(line!.height - 1.6 * 15)).toBeLessThan(2) // same line height as the rendered text
})

test('the bullet dot is centred on the first line of its block', async ({ backend, page }) => {
  backend.write('pages/t.md', '- first block\n- TODO second block\n')
  await backend.open('/page/t')
  for (const row of await page.getByRole('tree', { name: 'Blocks' }).locator('.row').all()) {
    // the drawn dot itself (the ::before of the button), not the button's box
    const dot = await row.locator('.bullet').evaluate((b) => {
      const r = b.getBoundingClientRect()
      const d = getComputedStyle(b, '::before')
      // the dot is laid out by flex centering: its middle is the button's middle when its box is centred;
      // measure it by drawing the position from a Range-free probe: temporarily make it an element
      const probe = document.createElement('span')
      probe.style.cssText = `display:block;width:${d.width};height:${d.height}`
      b.prepend(probe)
      const p = probe.getBoundingClientRect()
      probe.remove()
      return { top: p.top, height: p.height, btnTop: r.top, btnH: r.height }
    })
    const text = (await row.locator('.body').boundingBox())!
    const line = 1.6 * 15 // the first line of the block
    const dotMiddle = dot.top + dot.height / 2
    expect(Math.abs(dotMiddle - (text.y + line / 2))).toBeLessThan(1.5)
  }
})

// Enter right after typing, before the half-second save: no row with the text of the row above
test('typing in a new row and pressing Enter at once does not duplicate the row', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- first\n')
  await backend.open('/page/t')
  await page.waitForTimeout(600)
  await page.getByRole('tree', { name: 'Blocks' }).getByText('first', { exact: true }).click()
  await page.keyboard.press('End')
  const words = ['alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta']
  for (const [i, w] of words.entries()) {
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('block-editor')).toHaveText('')
    await page.keyboard.type(w, { delay: 20 })
    // the next Enter comes at once (i even) or after most of the save delay (i odd), but before the save
    await page.waitForTimeout(i % 2 === 0 ? 0 : 300)
  }
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- first\n' + words.map((w) => `- ${w}\n`).join(''))
})

// Alt+↓ / Alt+↑ held down (key repeat): every press moves the block one row, the editor stays on it
test('a block keeps moving while Alt+arrow is pressed again and again', async ({ backend, page }) => {
  backend.write('pages/t.md', '- a\n- DONE b\n  done:: 2026-10-03T10:00\n- c\n- d\n- e\n- f\n')
  await backend.open('/page/t')
  await page.waitForTimeout(600)
  await page.getByRole('tree', { name: 'Blocks' }).getByText('b', { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  for (let i = 0; i < 4; i++) await page.keyboard.press('Alt+ArrowDown') // no pause between the presses
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- a\n- c\n- d\n- e\n- f\n- DONE b\n  done:: 2026-10-03T10:00\n')
  await expect(page.getByTestId('block-editor')).toBeFocused()
  for (let i = 0; i < 5; i++) await page.keyboard.press('Alt+ArrowUp')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- DONE b\n  done:: 2026-10-03T10:00\n- a\n- c\n- d\n- e\n- f\n')
  await expect(page.getByTestId('block-editor')).toBeFocused()
})
