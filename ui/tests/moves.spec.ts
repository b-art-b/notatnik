import { expect, test } from './fixtures'

// Moving a block with Alt+↑/↓ again and again keeps the cursor in the block, in every browser engine. The desktop app
// runs in WebKit, which takes the focus away from a moved row without any event: the moves used to stop after one.
// `scripts/check-webkit.sh` runs this file in WebKit too.
const ROWS = Array.from({ length: 16 }, (_, i) => `- row${i}`).join('\n') + '\n'

for (const gap of [0, 40, 200]) {
  test(`Alt+↓ and Alt+↑ pressed every ${gap} ms keep moving the block, the cursor stays in it`, async ({
    backend,
    page,
  }) => {
    backend.write('pages/t.md', `- TODO target\n${ROWS}`)
    await backend.open('/page/t')
    await page.waitForTimeout(800)
    await page.getByRole('tree', { name: 'Blocks' }).getByText('target', { exact: true }).click()
    await expect(page.getByTestId('block-editor')).toBeFocused()
    for (let i = 0; i < 14; i++) {
      await page.keyboard.press('Alt+ArrowDown')
      await page.waitForTimeout(gap)
    }
    await expect
      .poll(() =>
        backend
          .read('pages/t.md')
          .split('\n')
          .findIndex((l) => l.includes('target')),
      )
      .toBe(14)
    await expect(page.getByTestId('block-editor')).toBeFocused()
    for (let i = 0; i < 14; i++) {
      await page.keyboard.press('Alt+ArrowUp')
      await page.waitForTimeout(gap)
    }
    await expect
      .poll(() =>
        backend
          .read('pages/t.md')
          .split('\n')
          .findIndex((l) => l.includes('target')),
      )
      .toBe(0)
    await expect(page.getByTestId('block-editor')).toBeFocused()
  })
}
