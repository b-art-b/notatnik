import type { Page } from '@playwright/test'
import { expect, test } from './fixtures'

// the row of a block (bullet + text); the bullet is the drag handle
const row = (page: Page, text: string) =>
  page.locator('.row', { has: page.getByText(text, { exact: true }) }).last()

test('drag a bullet below another one to reorder', async ({ backend, page }) => {
  backend.write('pages/d.md', '- A\n- B\n- C\n')
  await backend.open('/page/d')
  // lower half of C, left side: after C
  await row(page, 'A')
    .locator('.bullet')
    .dragTo(row(page, 'C'), { targetPosition: { x: 30, y: 22 } })
  await expect.poll(() => backend.read('pages/d.md')).toBe('- B\n- C\n- A\n')
  // upper half of B: before B
  await row(page, 'A')
    .locator('.bullet')
    .dragTo(row(page, 'B'), { targetPosition: { x: 30, y: 3 } })
  await expect.poll(() => backend.read('pages/d.md')).toBe('- A\n- B\n- C\n')
})

test('a dragged bullet takes its sub-bullets along', async ({ backend, page }) => {
  backend.write('pages/d.md', '- A\n  - a1\n    - a2\n- B\n')
  await backend.open('/page/d')
  await row(page, 'A')
    .locator('.bullet')
    .dragTo(row(page, 'B'), { targetPosition: { x: 30, y: 22 } })
  await expect.poll(() => backend.read('pages/d.md')).toBe('- B\n- A\n  - a1\n    - a2\n')
})

test('drop to the right of a bullet nests the block; drop on a parent makes it the first child', async ({
  backend,
  page,
}) => {
  backend.write('pages/d.md', '- A\n- B\n  - b1\n- C\n')
  await backend.open('/page/d')
  // lower half, far to the right of a leaf: child of A
  await row(page, 'C')
    .locator('.bullet')
    .dragTo(row(page, 'A'), { targetPosition: { x: 150, y: 22 } })
  await expect.poll(() => backend.read('pages/d.md')).toBe('- A\n  - C\n- B\n  - b1\n')
  // lower half of a block that shows children: first child
  await row(page, 'A')
    .locator('.bullet')
    .dragTo(row(page, 'B'), { targetPosition: { x: 30, y: 22 } })
  await expect.poll(() => backend.read('pages/d.md')).toBe('- B\n  - A\n    - C\n  - b1\n')
  // a nested block can move out to the top level
  await row(page, 'b1')
    .locator('.bullet')
    .dragTo(row(page, 'B'), { targetPosition: { x: 30, y: 3 } })
  await expect.poll(() => backend.read('pages/d.md')).toBe('- b1\n- B\n  - A\n    - C\n')
})

test('a block cannot be dropped into itself, and the page is left alone', async ({ backend, page }) => {
  backend.write('pages/d.md', '- A\n  - a1\n- B\n')
  await backend.open('/page/d')
  await row(page, 'A')
    .locator('.bullet')
    .dragTo(row(page, 'a1'), { targetPosition: { x: 30, y: 3 } })
  await page.waitForTimeout(300)
  expect(backend.read('pages/d.md')).toBe('- A\n  - a1\n- B\n')
  // no drop line is left behind
  await expect(page.locator('.row.drop-before, .row.drop-after, .row.drop-child')).toHaveCount(0)
})

test('undo brings a dragged block back', async ({ backend, page }) => {
  backend.write('pages/d.md', '- A\n- B\n- C\n')
  await backend.open('/page/d')
  await row(page, 'A')
    .locator('.bullet')
    .dragTo(row(page, 'C'), { targetPosition: { x: 30, y: 22 } })
  await expect.poll(() => backend.read('pages/d.md')).toBe('- B\n- C\n- A\n')
  await page.locator('body').click({ position: { x: 5, y: 5 } })
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/d.md')).toBe('- A\n- B\n- C\n')
})
