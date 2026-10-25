import { expect, test } from './fixtures'
import type { Page } from '@playwright/test'

// Which graph is this (MG-5): a color and the name, in the sidebar, the graph menu, the window title and, while two
// graphs are open, on every page.

/** The computed color the palette gives entry `n` (a probe element reads it through the CSS variable). */
const paletteColor = (page: Page, n: number) =>
  page.evaluate((i) => {
    const probe = document.createElement('div')
    probe.style.background = `var(--graph-${i})`
    document.body.append(probe)
    const c = getComputedStyle(probe).backgroundColor
    probe.remove()
    return c
  }, n)

test('the graph color shows in the sidebar, the menu and the title, and is chosen from the graph menu', async ({
  backend,
  page,
}) => {
  backend.write('pages/Colorful.md', '- x\n')
  await backend.open('/page/Colorful')
  const info = await backend.call<{ color: number; name: string }>('graph_info')
  const dot = page.getByTestId('graph-switcher').getByTestId('graph-dot')
  const bar = page.getByTestId('graph-section').first()
  await expect(dot).toHaveCSS('background-color', await paletteColor(page, info.color))
  await expect(bar).toHaveCSS('border-top-color', await paletteColor(page, info.color))
  await expect(bar).toHaveCSS('border-top-width', '3px')
  await expect(page).toHaveTitle(`Colorful — ${info.name}`)

  // another color from the menu: the dot, the bar and the settings follow
  const other = (info.color + 2) % 8
  await page.getByTestId('graph-switcher').click()
  await page.getByTestId('graph-menu-color').click()
  await page.getByTestId(`graph-color-${other}`).click()
  await expect(dot).toHaveCSS('background-color', await paletteColor(page, other))
  await expect(bar).toHaveCSS('border-top-color', await paletteColor(page, other))
  await expect(page.getByTestId('graph-color-name')).toHaveText(
    ['Blue', 'Green', 'Orange', 'Purple', 'Pink', 'Teal', 'Red', 'Gold'][other],
  )
  await expect(page.getByTestId(`graph-color-${other}`)).toHaveAttribute('aria-pressed', 'true')
  expect((await backend.call<{ color: number }>('graph_info')).color).toBe(other)
  // it stays after a reload
  await page.reload()
  await expect(page.getByTestId('graph-switcher').getByTestId('graph-dot')).toHaveCSS(
    'background-color',
    await paletteColor(page, other),
  )
  // and "Automatic" goes back to the color of the graph's ID
  await page.getByTestId('graph-switcher').click()
  await page.getByTestId('graph-menu-color').click()
  await page.getByTestId('graph-color-reset').click()
  await expect(page.getByTestId('graph-switcher').getByTestId('graph-dot')).toHaveCSS(
    'background-color',
    await paletteColor(page, info.color),
  )
})

test('a page names its graph only while two graphs are open', async ({ backend, page }) => {
  backend.write('pages/Chipped.md', '- x\n')
  await backend.open('/page/Chipped')
  await expect(page.getByRole('main').getByRole('heading', { name: 'Chipped' })).toBeVisible()
  await expect(page.getByTestId('graph-chip')).toHaveCount(0)

  await backend.beside('Beta')
  await page.reload()
  const chip = page.getByRole('main').getByTestId('graph-chip')
  await expect(chip).toBeVisible()
  await expect(chip).toContainText('graph')
  // the color is a tint of the graph's color, and the name is there too (the color is never the only cue)
  const info = await backend.call<{ color: number }>('graph_info')
  await expect(chip.locator('.gdot')).toHaveCSS('background-color', await paletteColor(page, info.color))
})

test('the dialog shows both graphs with their colors', async ({ backend, page }) => {
  backend.write('pages/Dotted.md', '- x\n')
  const beta = await backend.beside('Beta')
  await backend.open('/page/Dotted')
  await page.getByRole('main').getByTestId('transfer-page').click()
  const target = page.getByTestId('transfer-dialog').locator('label.choice', { hasText: 'Beta' })
  const color = (await beta.call<{ color: number }>('graph_info')).color
  await expect(target.locator('.gdot')).toHaveCSS('background-color', await paletteColor(page, color))
  await expect(target).toContainText('second graph')
})
