import { expect, test } from './fixtures'

// LS-1..LS-6: the children of a block as bullets, numbers, letters or roman numerals (`list::` on the parent).
const PAGE = '- parent\n- other\n  - x\n'
const FILE = 'pages/ls.md'

async function chooseFromMenu(page: import('@playwright/test').Page, bullet: number, item: RegExp) {
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).nth(bullet).click({ button: 'right' })
  await page.getByTestId('block-menu').getByRole('menuitem', { name: item }).click()
}

const marks = (page: import('@playwright/test').Page) =>
  page
    .getByRole('tree', { name: 'Blocks' })
    .getByRole('button', { name: 'Zoom into block' })
    .allTextContents()

test('each style from the bullet menu: the file, the marks, undo', async ({ backend, page }) => {
  backend.write(FILE, '- parent\n  - one\n  - two\n  - three\n- other\n')
  await backend.open('/page/ls')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await expect(blocks.getByText('three')).toBeVisible()
  expect(await marks(page)).toEqual(['●', '●', '●', '●', '●'])

  await chooseFromMenu(page, 0, /Children as numbered/)
  await expect
    .poll(() => backend.read(FILE))
    .toBe('- parent\n  list:: numbered\n  - one\n  - two\n  - three\n- other\n')
  await expect.poll(() => marks(page)).toEqual(['●', '1.', '2.', '3.', '●'])
  // the property is not shown in the property card
  await expect(blocks.getByText('list', { exact: true })).toHaveCount(0)

  await chooseFromMenu(page, 0, /Children as letters/)
  await expect.poll(() => marks(page)).toEqual(['●', 'a.', 'b.', 'c.', '●'])
  await chooseFromMenu(page, 0, /Children as roman numerals/)
  await expect.poll(() => marks(page)).toEqual(['●', 'i.', 'ii.', 'iii.', '●'])
  expect(backend.read(FILE)).toContain('list:: roman')

  // the current style is ticked
  await blocks.getByRole('button', { name: 'Zoom into block' }).first().click({ button: 'right' })
  await expect(page.getByRole('menuitem', { name: /Children as roman numerals/ })).toContainText('✓')
  await page.keyboard.press('Escape')

  // Bullets removes the property; undo brings the last style back
  await chooseFromMenu(page, 0, /Children as bullets/)
  await expect.poll(() => backend.read(FILE)).toBe('- parent\n  - one\n  - two\n  - three\n- other\n')
  await expect.poll(() => marks(page)).toEqual(['●', '●', '●', '●', '●'])
  await page.keyboard.press('Control+z')
  await expect.poll(() => marks(page)).toEqual(['●', 'i.', 'ii.', 'iii.', '●'])
})

test('the marks follow the blocks, restart under each parent and do not reach grandchildren', async ({
  backend,
  page,
}) => {
  backend.write(
    FILE,
    '- parent\n  list:: numbered\n  - one\n    - deep a\n    - deep b\n  - two\n  - three\n- second\n  list:: numbered\n  - solo\n',
  )
  await backend.open('/page/ls')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await expect(blocks.getByText('solo')).toBeVisible()
  // parent, one, deep a, deep b, two, three, second, solo
  expect(await marks(page)).toEqual(['●', '1.', '●', '●', '2.', '3.', '●', '1.'])

  // delete the first child: the others renumber
  await blocks.getByText('one', { exact: true }).click()
  await page.keyboard.press('Escape')
  await page.keyboard.press('Delete')
  await expect.poll(() => marks(page)).toEqual(['●', '1.', '2.', '●', '1.'])
  // undo
  await page.keyboard.press('Control+z')
  await expect.poll(() => marks(page)).toEqual(['●', '1.', '●', '●', '2.', '3.', '●', '1.'])
  // a move renumbers too
  await blocks.getByText('three', { exact: true }).click()
  await page.keyboard.press('Alt+ArrowUp')
  await page.keyboard.press('Alt+ArrowUp')
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read(FILE)).toMatch(/- three\n {2}- one/)
  await expect.poll(() => marks(page)).toEqual(['●', '1.', '2.', '●', '●', '3.', '●', '1.'])
})

test('the palette sets the style of the selected block, by keyboard only', async ({ backend, page }) => {
  backend.write(FILE, '- parent\n  - one\n  - two\n')
  await backend.open('/page/ls')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByText('parent', { exact: true }).click()
  await page.keyboard.press('Escape') // selects the block
  await page.keyboard.press('Control+k')
  const palette = page.getByRole('dialog', { name: 'Command palette' })
  await palette.getByLabel('Command or page').fill('children as letters')
  await page.keyboard.press('Enter')
  await expect(palette).toHaveCount(0)
  await expect.poll(() => backend.read(FILE)).toBe('- parent\n  list:: letters\n  - one\n  - two\n')
  await expect.poll(() => marks(page)).toEqual(['●', 'a.', 'b.'])
})

test('the file with list:: is read back unchanged, and another value is the bullet dot', async ({
  backend,
  page,
}) => {
  const text = '- parent\n  list:: roman\n  - one\n- odd\n  list:: stars\n  - x\n'
  backend.write(FILE, text)
  await backend.open('/page/ls')
  await expect(page.getByRole('tree', { name: 'Blocks' }).getByText('one')).toBeVisible()
  expect(await marks(page)).toEqual(['●', 'i.', '●', '●'])
  await page.waitForTimeout(500)
  expect(backend.read(FILE)).toBe(text)
})

test('the marks do not move the text column, in a wide and a narrow window', async ({ backend, page }) => {
  backend.write(
    FILE,
    '- parent\n  list:: roman\n  - one\n  - two\n  - three\n  - four\n  - five\n  - six\n  - seven\n  - eight\n',
  )
  for (const width of [1850, 700]) {
    await page.setViewportSize({ width, height: 900 })
    await backend.open('/page/ls')
    const blocks = page.getByRole('tree', { name: 'Blocks' })
    await expect(blocks.getByText('eight')).toBeVisible()
    const xs = await Promise.all(
      ['one', 'three', 'eight'].map(
        async (t) => (await blocks.getByText(t, { exact: true }).boundingBox())!.x,
      ),
    )
    expect(Math.max(...xs) - Math.min(...xs)).toBeLessThan(1)
    // `viii.` stays inside the page, left of its text
    const mark = blocks.locator('.mark').last()
    await expect(mark).toHaveText('viii.')
    const m = (await mark.boundingBox())!
    expect(m.x).toBeGreaterThanOrEqual(0)
    expect(m.x + m.width).toBeLessThanOrEqual(xs[2])
  }
})

test('Copy with children keeps the style, so a paste brings it back', async ({ backend, page }) => {
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
  backend.write(FILE, '- parent\n  list:: letters\n  - one\n  - two\n')
  await backend.open('/page/ls')
  await chooseFromMenu(page, 0, /Copy with children/)
  const copied = await page.evaluate(() => (window as unknown as { copied: string[] }).copied.at(-1))
  expect(copied).toBe('- parent\n  list:: letters\n  - one\n  - two')
})
