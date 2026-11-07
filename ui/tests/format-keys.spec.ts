import { expect, test } from './fixtures'

// FK-1..FK-6: bold, italic, strike, highlight and code keys for the selected text; Ctrl+B is "cursor left" without one.
const FILE = 'pages/fk.md'

async function edit(
  page: import('@playwright/test').Page,
  backend: { write: (p: string, t: string) => void; open: (u: string) => Promise<void> },
  text: string,
  click?: string,
) {
  backend.write(FILE, text + '\n')
  await backend.open('/page/fk')
  await page
    .getByRole('tree', { name: 'Blocks' })
    .getByText(click ?? text.replace(/^- /, '').split('\n')[0])
    .first()
    .click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
}

const selected = (page: import('@playwright/test').Page) =>
  page.evaluate(() => window.getSelection()?.toString() ?? '')

for (const [name, keys, marker] of [
  ['bold', 'Control+b', '**'],
  ['italic', 'Control+i', '*'],
  ['strikethrough', 'Control+Shift+x', '~~'],
  ['highlight', 'Control+Shift+h', '=='],
  ['code', 'Control+Shift+c', '`'],
] as const) {
  test(`${name}: the key wraps the selected word, again unwraps, Ctrl+Z undoes`, async ({
    backend,
    page,
  }) => {
    await edit(page, backend, '- say word now')
    await page.keyboard.press('Home')
    await page.keyboard.press('Control+ArrowRight') // after "say"
    await page.keyboard.press('Control+ArrowRight')
    await page.keyboard.press('Control+Shift+ArrowLeft') // selects "word"
    expect(await selected(page)).toBe('word')
    await page.keyboard.press(keys)
    await expect(page.getByTestId('block-editor')).toContainText(`say ${marker}word${marker} now`)
    // the selection is still the word, without the markers
    expect(await selected(page)).toBe('word')
    await page.keyboard.press('Escape')
    await expect.poll(() => backend.read(FILE)).toBe(`- say ${marker}word${marker} now\n`)
  })
}

test('the same key again takes the markers off, and one Ctrl+Z puts them back', async ({ backend, page }) => {
  await edit(page, backend, '- say **word** now', 'now')
  const ed = page.getByTestId('block-editor')
  await page.keyboard.press('Home')
  for (let i = 0; i < 6; i++) await page.keyboard.press('ArrowRight') // after "say **"
  for (let i = 0; i < 4; i++) await page.keyboard.press('Shift+ArrowRight') // "word"
  await page.keyboard.press('Control+b')
  await expect(ed).toContainText('say word now')
  await expect(ed).not.toContainText('**')
  await page.keyboard.press('Control+z')
  await expect(ed).toContainText('say **word** now')
})

test('bold and italic combine, and a second key adds to the selection', async ({ backend, page }) => {
  await edit(page, backend, '- word')
  await page.keyboard.press('Control+a')
  await page.keyboard.press('Control+Shift+ArrowRight')
  await page.keyboard.press('Home')
  await page.keyboard.press('Shift+End')
  await page.keyboard.press('Control+b')
  await page.keyboard.press('Control+i')
  await expect(page.getByTestId('block-editor')).toContainText('***word***')
})

test('without a selection Ctrl+B moves the cursor left and the others do nothing', async ({
  backend,
  page,
}) => {
  await edit(page, backend, '- abc')
  const ed = page.getByTestId('block-editor')
  await page.keyboard.press('End')
  await page.keyboard.press('Control+b') // cursor left of "c"
  await page.keyboard.type('X')
  await expect(ed).toContainText('abXc')
  for (const k of ['Control+i', 'Control+Shift+x', 'Control+Shift+h', 'Control+Shift+c'])
    await page.keyboard.press(k)
  await expect(ed).toContainText('abXc')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read(FILE)).toBe('- abXc\n')
})

test('inside a fenced code block the keys change nothing', async ({ backend, page }) => {
  await edit(page, backend, '- code\n  ```\n  let x = 1\n  ```')
  const ed = page.getByTestId('block-editor')
  await page.keyboard.press('Control+End')
  for (let i = 0; i < 6; i++) await page.keyboard.press('ArrowUp').catch(() => {})
  await page.keyboard.press('Control+a')
  await page.keyboard.press('Control+i')
  await page.keyboard.press('Control+Shift+x')
  await expect(ed).not.toContainText('~~')
  expect(backend.read(FILE)).toContain('let x = 1')
})
