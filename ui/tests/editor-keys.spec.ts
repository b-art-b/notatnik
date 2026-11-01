import { expect, test } from './fixtures'

// Emacs / readline keys in a block: Ctrl+A and Ctrl+E (line start and end), Ctrl+W (a word back), Alt+B and Alt+F
// (a word left and right), Alt+D (a word forward), Ctrl+U (to the line start), Ctrl+B and Ctrl+F (a character),
// Ctrl+P and Ctrl+N (a line up and down, over the edge into the neighbour block).
async function open(
  page: import('@playwright/test').Page,
  backend: { write: (p: string, t: string) => void; open: (r: string) => Promise<void> },
  text: string,
  click: string,
) {
  backend.write('pages/k.md', text)
  await backend.open('/page/k')
  await page.getByRole('tree', { name: 'Blocks' }).getByText(click, { exact: true }).click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
}
const editor = (page: import('@playwright/test').Page) => page.getByTestId('block-editor')

test('Ctrl+A goes to the start of the line, Ctrl+E to the end', async ({ backend, page }) => {
  await open(page, backend, '- hello brave world\n', 'hello brave world')
  await page.keyboard.press('Control+a')
  await page.keyboard.type('>')
  await page.keyboard.press('Control+e')
  await page.keyboard.type('<')
  await expect(editor(page)).toHaveText('>hello brave world<')
})

test('Ctrl+W deletes a word back, Alt+D a word forward, Ctrl+U up to the line start', async ({
  backend,
  page,
}) => {
  await open(page, backend, '- one two three four\n', 'one two three four')
  await page.keyboard.press('Control+e')
  await page.keyboard.press('Control+w')
  await expect(editor(page)).toHaveText('one two three ')
  await page.keyboard.press('Alt+b')
  await page.keyboard.press('Alt+b')
  await page.keyboard.press('Alt+d')
  await expect(editor(page)).toHaveText('one  three ')
  await page.keyboard.press('Control+u')
  await expect(editor(page)).toHaveText(' three ')
})

test('Alt+B / Alt+F and Ctrl+B / Ctrl+F move by word and by character', async ({ backend, page }) => {
  await open(page, backend, '- one two\n', 'one two')
  await page.keyboard.press('Control+e')
  await page.keyboard.press('Alt+b')
  await page.keyboard.type('|')
  await page.keyboard.press('Control+b')
  await page.keyboard.type('(')
  await page.keyboard.press('Control+f')
  await page.keyboard.press('Control+f')
  await page.keyboard.press('Alt+f')
  await page.keyboard.type(')')
  await expect(editor(page)).toHaveText('one (|two)')
})

test('Ctrl+P and Ctrl+N move between blocks like the arrows', async ({ backend, page }) => {
  await open(page, backend, '- first\n- second\n', 'second')
  await page.keyboard.press('Control+p')
  await expect(editor(page)).toContainText('first')
  await page.keyboard.press('Control+n')
  await expect(editor(page)).toContainText('second')
})

test('Ctrl+Shift+A still selects the text of the block', async ({ backend, page }) => {
  await open(page, backend, '- replace me\n', 'replace me')
  await page.keyboard.press('Control+Shift+a')
  await page.keyboard.type('done')
  await expect(editor(page)).toHaveText('done')
})
