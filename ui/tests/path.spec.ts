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
