import { expect, test } from './fixtures'

test('the path to the edited block is marked: ancestors and the block, nothing else', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/t.md',
    '- root\n  - mid\n    - first\n    - target\n    - last\n  - other\n- second root\n',
  )
  await backend.open('/page/t')
  await expect(page.locator('.block.on-path')).toHaveCount(0)
  await page.getByText('target').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  // root, mid and target are on the path; first/last/other/second root are not
  await expect(page.locator('.block.on-path')).toHaveCount(3)
  const marked = await page
    .locator('.block.on-path > .row')
    .evaluateAll((rows) => rows.map((r) => r.textContent?.replace(/[▾▸●]/g, '').trim().split(/\s+/)[0]))
  expect(marked).toEqual(['root', 'mid', 'target'])
  // moving up to the previous block moves the path
  await page.keyboard.press('ArrowUp')
  await expect(page.locator('.block.on-path > .row').last()).toContainText('first')
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await expect(page.locator('.block.on-path')).toHaveCount(0)
})

test('the elbow reaches the dot of its block and the arrow does not sit on it', async ({ backend, page }) => {
  backend.write('pages/t.md', '- root\n  - mid\n    - target\n')
  await backend.open('/page/t')
  await page.getByText('target').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  const mid = page.locator('.block.on-path').nth(1)
  const m = await mid.evaluate((el) => {
    const dot = el.querySelector(':scope > .row > .bullet')!.getBoundingClientRect()
    const box = el.getBoundingClientRect()
    const elbow = getComputedStyle(el, '::before')
    // the dot is placed by position, not by a flex <button> (WebKit does not centre it that way)
    const dotPos = getComputedStyle(el.querySelector(':scope > .row > .bullet')!, '::before').position
    // the elbow starts 21px left of the block and ends at its right edge
    return { end: box.left - 21 + parseFloat(elbow.width), dotLeft: dot.left + (dot.width - 6) / 2, dotPos }
  })
  expect(m.dotPos).toBe('absolute')
  // ends before the dot's left edge, within 6px of it
  expect(m.end).toBeLessThanOrEqual(m.dotLeft)
  expect(m.dotLeft - m.end).toBeLessThan(6)
  // the expand arrow of a block on the path stays hidden while the pointer is elsewhere
  await page.mouse.move(700, 500)
  await expect(mid.locator(':scope > .row > .arrow')).toHaveCSS('opacity', '0')
  await mid.locator(':scope > .row > .arrow').hover()
  await expect(mid.locator(':scope > .row > .arrow')).toHaveCSS('opacity', '1')
})
