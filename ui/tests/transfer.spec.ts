import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from './fixtures'

// Taking pages and blocks to another graph (MG-2 to MG-4). The second graph is open beside the first in the app; the
// dialog is the first graph's.
const dialog = (page: import('@playwright/test').Page) => page.getByTestId('transfer-dialog')

test('a page is copied to the other graph with its picture, then moved, then taken back', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  const pic = 'assets/2026/10/pic-11111111.png'
  fs.mkdirSync(path.dirname(path.join(backend.dir, pic)), { recursive: true })
  fs.writeFileSync(path.join(backend.dir, pic), 'png bytes')
  backend.write('pages/Traveller.md', `- text with [[Nowhere]]\n- ![](../${pic})\n`)
  await backend.open('/page/Traveller')

  await page.getByRole('main').getByTestId('transfer-page').click()
  const d = dialog(page)
  await expect(d.getByTestId('transfer-target')).toHaveCount(1)
  await expect(d.getByTestId('transfer-plan')).toContainText('New page Traveller')
  await expect(d.getByTestId('transfer-plan')).toContainText('1 picture or file go along')
  await expect(d.getByTestId('transfer-links')).toContainText('Beta has no page for: Nowhere')
  await d.getByTestId('transfer-go').click()
  await expect(d).toHaveCount(0)
  await expect(page.locator('.toast', { hasText: 'Copied “Traveller” to Beta' })).toBeVisible()
  expect(beta.read('pages/Traveller.md')).toBe(backend.read('pages/Traveller.md'))
  expect(beta.read(pic)).toBe('png bytes')
  expect(backend.exists('pages/Traveller.md'), 'a copy leaves the page where it is').toBe(true)

  // moving it again: the name exists in Beta now, so both are kept
  await page.getByRole('main').getByTestId('transfer-page').click()
  await d.getByTestId('transfer-move').check()
  await expect(d.getByTestId('transfer-plan')).toContainText('New page Traveller (from graph)')
  await expect(d.getByTestId('transfer-clash')).toBeVisible()
  await d.getByTestId('transfer-go').click()
  await expect(page.locator('.toast', { hasText: 'Moved' })).toBeVisible()
  expect(backend.exists('pages/Traveller.md')).toBe(false)
  expect(beta.exists('pages/Traveller (from graph).md')).toBe(true)
  // the page that left is not on screen any more
  await expect(page).toHaveURL(/#\/?$|#\/journals/)

  await page.locator('.toast', { hasText: 'Moved' }).getByRole('button', { name: 'Undo' }).click()
  await expect.poll(() => backend.exists('pages/Traveller.md')).toBe(true)
  expect(beta.exists('pages/Traveller (from graph).md')).toBe(false)
})

test('blocks of a page go to the end of a page in the other graph, from the bullet menu', async ({
  backend,
  page,
}) => {
  const beta = await backend.beside('Beta')
  beta.write('pages/Inbox.md', '- already there\n')
  backend.write('pages/Notes.md', '- keep\n- take me\n  - and my child\n')
  await backend.call('get_page', { name: 'Notes' })
  await beta.call('get_page', { name: 'Inbox' })
  await backend.open('/page/Notes')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByRole('button', { name: 'Zoom into block' }).nth(1).click({ button: 'right' })
  await page.getByTestId('block-menu').getByRole('menuitem', { name: 'To another graph…' }).click()
  const d = dialog(page)
  await d.getByTestId('transfer-page-name').fill('Inbox')
  await expect(d.getByTestId('transfer-plan')).toContainText('Added to Inbox')
  await d.getByTestId('transfer-move').check()
  await d.getByTestId('transfer-go').click()
  await expect(page.locator('.toast', { hasText: 'Moved 1 block to Beta' })).toBeVisible()
  expect(beta.read('pages/Inbox.md')).toBe('- already there\n- take me\n  - and my child\n')
  await expect.poll(() => backend.read('pages/Notes.md')).toBe('- keep\n')
  // the block is gone from the page on screen as well
  await expect(blocks.getByText('take me')).toHaveCount(0)
  await expect(blocks.getByText('keep')).toBeVisible()
})

test('the button is not there while there is only one graph, and the palette says so', async ({
  backend,
  page,
}) => {
  backend.write('pages/Alone.md', '- x\n')
  await backend.open('/page/Alone')
  await expect(page.getByRole('main').getByTestId('transfer-page')).toHaveCount(0)
  await page.keyboard.press('Control+k')
  await page.getByRole('dialog').getByRole('textbox').fill('to another graph')
  await page.keyboard.press('Enter')
  await expect(dialog(page).getByTestId('transfer-none')).toBeVisible()
  await expect(dialog(page).getByTestId('transfer-go')).toBeDisabled()
})
