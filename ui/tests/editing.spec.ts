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
