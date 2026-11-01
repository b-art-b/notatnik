import type { Page } from '@playwright/test'
import { expect, test } from './fixtures'

// Selecting several blocks: with the mouse and the keyboard, then delete, copy, cut, paste, duplicate, move.
const TREE = '- a\n  - a1\n  - a2\n- b\n- c\n  - c1\n- d\n'

const blocks = (page: Page) => page.getByRole('tree', { name: 'Blocks' })
const selected = (page: Page) => blocks(page).getByRole('treeitem', { selected: true })
const text = (page: Page, t: string) => blocks(page).getByText(t, { exact: true }).first()

/** Copies go to the page instead of the shared system clipboard (tests run in parallel), and a paste is sent as
 * the event a Ctrl+V would raise. */
async function fakeClipboard(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as { copied: string[] }
    w.copied = []
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        writeText: (t: string) => {
          w.copied.push(t)
          return Promise.resolve()
        },
      },
    })
  })
}
const lastCopy = (page: Page) =>
  page.evaluate(() => (window as unknown as { copied: string[] }).copied.at(-1) ?? null)
const pasteText = (page: Page, t: string) =>
  page.evaluate((t) => {
    const data = new DataTransfer()
    data.setData('text/plain', t)
    window.dispatchEvent(
      new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true }),
    )
  }, t)

/** The file watcher reports the test's own write ~200 ms after the page loaded and the page reloads then: wait it out,
 * so a click does not land on blocks that are about to be replaced. */
const settle = (page: Page) => page.waitForTimeout(500)

/** Selects one block: click it, Esc. Retried as a whole: while the page is still being reloaded after a write the
 * editor may belong to a block the page has just replaced. */
async function pick(page: Page, t: string) {
  // a block whose text has more than the plain word (a reference after it) is found by what it starts with
  const target = t === 'see' ? blocks(page).getByText(/^see/).first() : text(page, t)
  await expect(async () => {
    await page.keyboard.press('Escape')
    await target.click({ position: t === 'see' ? { x: 4, y: 4 } : undefined })
    await expect(page.getByTestId('block-editor')).toBeFocused({ timeout: 1500 })
    await page.keyboard.press('Escape')
    await expect(selected(page)).toHaveCount(1, { timeout: 1500 })
  }).toPass({ timeout: 10000 })
}

test('Ctrl+Shift+A in a block selects its text, again every block of the page', async ({ backend, page }) => {
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  await settle(page)
  await text(page, 'b').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Control+Shift+a')
  await expect(selected(page)).toHaveCount(0) // the text of the block, nothing more
  await page.keyboard.press('Control+Shift+a')
  await expect(selected(page)).toHaveCount(7)
  // and from a selection, Ctrl+A selects all as well
  await page.keyboard.press('Escape')
  await pick(page, 'a1')
  await page.keyboard.press('Control+a')
  await expect(selected(page)).toHaveCount(7)
})

test('Shift+click selects a range as on screen; Delete removes it in one undo step', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  await settle(page)
  await text(page, 'a').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await text(page, 'c').click({ modifiers: ['Shift'] })
  await expect(selected(page)).toHaveCount(5) // a a1 a2 b c (c1 comes along as c's child)
  await page.keyboard.press('Delete')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- d\n')
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/t.md')).toBe(TREE)
})

test('Ctrl+click adds and removes blocks; a block takes its children along', async ({ backend, page }) => {
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  await settle(page)
  await pick(page, 'b')
  await text(page, 'a').click({ modifiers: ['Control'] })
  await expect(selected(page)).toHaveCount(4) // b, a and its two children
  await text(page, 'd').click({ modifiers: ['Control'] })
  await expect(selected(page)).toHaveCount(5)
  await text(page, 'b').click({ modifiers: ['Control'] })
  await expect(selected(page)).toHaveCount(4)
  await page.keyboard.press('Delete')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- b\n- c\n  - c1\n')
})

test('dragging across blocks selects them; a drag inside one block stays a text selection', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  await settle(page)
  const from = (await text(page, 'a2').boundingBox())!
  const to = (await text(page, 'c').boundingBox())!
  await page.mouse.move(from.x + 3, from.y + from.height / 2)
  await page.mouse.down()
  await page.mouse.move(to.x + 3, to.y + to.height / 2, { steps: 8 })
  await page.mouse.up()
  await expect(selected(page)).toHaveCount(3) // a2 b c
  await expect(page.getByTestId('block-editor')).toHaveCount(0) // the click that ended the drag opened nothing
  await page.keyboard.press('Delete')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n  - a1\n- d\n')
})

test('copy and paste: the blocks come back below the selection with their structure, without block IDs', async ({
  backend,
  page,
}) => {
  await fakeClipboard(page)
  backend.write('pages/t.md', '- a ^aaa111\n  - a1\n- b\n- c\n')
  await backend.open('/page/t')
  await settle(page)
  await pick(page, 'a')
  await page.keyboard.press('Control+c')
  await expect.poll(() => lastCopy(page)).toBe('- a\n  - a1')
  await pick(page, 'c')
  await pasteText(page, (await lastCopy(page))!)
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a ^aaa111\n  - a1\n- b\n- c\n- a\n  - a1\n')
  // the pasted blocks are selected
  await expect(selected(page)).toHaveCount(1)
})

test('cut and paste moves blocks and keeps their IDs, so references survive', async ({ backend, page }) => {
  await fakeClipboard(page)
  backend.write('pages/t.md', '- a ^aaa111\n- b\n- see ((aaa111))\n')
  await backend.open('/page/t')
  await settle(page)
  await pick(page, 'b')
  await page.keyboard.press('Control+x')
  await expect.poll(() => backend.read('pages/t.md')).not.toContain('- b')
  await expect(selected(page)).toHaveCount(0)
  await pick(page, 'a')
  await pasteText(page, (await lastCopy(page))!)
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a ^aaa111\n- b\n- see ((aaa111))\n')
  await expect(selected(page)).toHaveCount(1) // the pasted block
  // cut the block that is referenced and paste it elsewhere: same ID, one block
  await pick(page, 'a')
  await page.keyboard.press('Control+x')
  await expect.poll(() => backend.read('pages/t.md')).not.toContain('^aaa111')
  await expect(selected(page)).toHaveCount(0)
  await pick(page, 'see')
  await pasteText(page, (await lastCopy(page))!)
  await expect.poll(() => backend.read('pages/t.md')).toBe('- b\n- see ((aaa111))\n- a ^aaa111\n')
})

test('Ctrl+D duplicates, Alt+↓ moves several blocks together, Ctrl+Enter cycles every task', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  await settle(page)
  await pick(page, 'a')
  await page.keyboard.press('Control+d')
  await expect
    .poll(() => backend.read('pages/t.md'))
    .toBe('- a\n  - a1\n  - a2\n- a\n  - a1\n  - a2\n- b\n- c\n  - c1\n- d\n')
  await expect(selected(page)).toHaveCount(1) // the copy
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/t.md')).toBe(TREE)
  // b and c together below d
  await pick(page, 'b')
  await text(page, 'c').click({ modifiers: ['Shift'] })
  await page.keyboard.press('Alt+ArrowDown')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n  - a1\n  - a2\n- d\n- b\n- c\n  - c1\n')
  await expect(selected(page)).toHaveCount(2) // still selected after the move
  await settle(page)
  // every selected block becomes a task
  await page.keyboard.press('Control+Enter')
  await expect.poll(() => backend.read('pages/t.md')).toContain('- TODO b\n- TODO c\n')
})

test('the right-click menu of a selection copies, duplicates and deletes it', async ({ backend, page }) => {
  await fakeClipboard(page)
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  await settle(page)
  await pick(page, 'b')
  await text(page, 'd').click({ modifiers: ['Shift'] })
  await expect(selected(page)).toHaveCount(4) // b c c1 d
  await text(page, 'c').click({ button: 'right' })
  const menu = page.getByTestId('block-menu')
  await expect(page.getByTestId('block-menu-id')).toHaveText('4 blocks selected')
  await expect(menu.getByRole('menuitem')).toHaveText([
    /Copy/,
    /Cut/,
    /Duplicate/,
    /Copy block references/,
    /Save as template/,
    /Delete/,
  ])
  await menu.getByRole('menuitem', { name: 'Copy Ctrl+C' }).click()
  await expect.poll(() => lastCopy(page)).toBe('- b\n- c\n  - c1\n- d')
  await text(page, 'c').click({ button: 'right' })
  await menu.getByRole('menuitem', { name: /Delete/ }).click()
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n  - a1\n  - a2\n')
})

// org-mode style: Alt+arrows move the block (with its children) in every direction
test('Alt+arrows move a block like org-mode: up/down among siblings, left/right between levels', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- a\n- b\n  - b1\n- c\n')
  await backend.open('/page/t')
  await settle(page)
  await text(page, 'b').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Alt+ArrowRight') // demote: b (with b1) goes under a
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n  - b\n    - b1\n- c\n')
  await settle(page)
  await page.keyboard.press('Alt+ArrowLeft') // promote again
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n- b\n  - b1\n- c\n')
  await settle(page) // the editor comes back on the moved block
  await page.keyboard.press('Alt+ArrowDown')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n- c\n- b\n  - b1\n')
  await settle(page)
  await page.keyboard.press('Alt+ArrowUp')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n- b\n  - b1\n- c\n')
  await settle(page)
  // the same on selected blocks, where Alt+← / Alt+→ do not go back / forward in the page history
  await page.keyboard.press('Escape')
  await expect(selected(page)).toHaveCount(1)
  await page.keyboard.press('Alt+ArrowRight')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n  - b\n    - b1\n- c\n')
  await expect(selected(page)).toHaveCount(1)
  await settle(page)
  await page.keyboard.press('Alt+ArrowLeft')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n- b\n  - b1\n- c\n')
  await expect(page).toHaveURL(/#\/page\/t/)
})

test('text marked with the mouse stays marked (to copy it); a plain click still edits', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- hello wonderful world\n- other\n')
  await backend.open('/page/t')
  await settle(page)
  const box = (await text(page, 'hello wonderful world').boundingBox())!
  // drag from the start of the text to the end of the word "wonderful"
  await page.mouse.move(box.x + 2, box.y + box.height / 2)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width * 0.75, box.y + box.height / 2, { steps: 8 })
  await page.mouse.up()
  await expect(page.getByTestId('block-editor')).toHaveCount(0) // still the rendered block
  const marked = await page.evaluate(() => window.getSelection()?.toString() ?? '')
  expect(marked.length).toBeGreaterThan(5)
  expect('hello wonderful world').toContain(marked.trim())
  // a click elsewhere in the text clears the marking and edits
  await text(page, 'other').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
})

test('Shift+Tab outdents a selection that spans levels; top-level blocks stay', async ({ backend, page }) => {
  backend.write('pages/t.md', TREE)
  await backend.open('/page/t')
  await settle(page)
  await text(page, 'a1').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await text(page, 'b').click({ modifiers: ['Shift'] })
  await expect(selected(page)).toHaveCount(3) // a1 a2 b
  await page.keyboard.press('Shift+Tab')
  await expect.poll(() => backend.read('pages/t.md')).toBe('- a\n- a1\n- a2\n- b\n- c\n  - c1\n- d\n')
  await page.keyboard.press('Control+z')
  await expect.poll(() => backend.read('pages/t.md')).toBe(TREE)
})
