import { expect, test } from './fixtures'

// A page of a single journal day: the date once, one row of actions in one style on the title's left edge, no
// Hierarchy (a day has no subpages), and an empty day that shows where the text will go.
test('a journal day shows its date once and its actions in one aligned row', async ({ backend, page }) => {
  backend.write('journals/2026/2026-10-10.md', '- a day\n')
  await backend.open('/page/2026-10-10')
  const main = page.getByRole('main')
  const title = main.locator('h1')
  // a title that does not show the page name (a short date) gets it beside it, small and not bold
  await expect(title.locator('.iso')).toHaveText('2026-10-10')
  expect(await title.locator('.iso').evaluate((e) => getComputedStyle(e).fontWeight)).toBe('400')

  // an ISO title shows the name already: it is not repeated, and is the tooltip instead
  await backend.call('set_settings', { date_format: 'iso' })
  await page.reload()
  await expect(title).toContainText('2026-10-10')
  await expect(title.locator('.iso')).toHaveCount(0)
  await expect(title.locator('button.title, a.title')).toHaveAttribute('title', /Page name: 2026-10-10/)

  // History, Encryption, Favorite and Properties look the same and start where the title starts
  const actions = main.locator('.page-actions')
  const buttons = actions.getByRole('button')
  await expect(buttons).toHaveCount(4)
  const sizes = await buttons.evaluateAll((els) =>
    els.map((e) => {
      const cs = getComputedStyle(e)
      return [cs.fontSize, cs.color, cs.paddingLeft]
    }),
  )
  expect(new Set(sizes.map((s) => s.join('|'))).size).toBe(1)
  const textLeft = async (loc: ReturnType<typeof main.locator>) =>
    loc.evaluate((e) => {
      const r = document.createRange()
      r.selectNodeContents(e)
      return r.getBoundingClientRect().left
    })
  const titleLeft = await textLeft(title.locator('.title'))
  expect(Math.abs((await textLeft(buttons.first())) - titleLeft)).toBeLessThan(2)

  // a day has no subpages: no Hierarchy block
  await expect(page.getByTestId('hierarchy')).toHaveCount(0)
})

test('an empty page shows a faint bullet where the text will go; a page without subpages shows only a quiet add link', async ({
  backend,
  page,
}) => {
  backend.write('pages/Solo.md', '')
  await backend.open('/page/Solo')
  const empty = page.getByRole('button', { name: 'Click here to start writing' })
  await expect(empty).toBeVisible()
  const bullet = await empty.evaluate((e) => {
    const cs = getComputedStyle(e, '::before')
    return { width: cs.width, opacity: cs.opacity, content: cs.content }
  })
  expect(bullet).toEqual({ width: '6px', opacity: '0.4', content: '""' })

  // no heading and no list: just the way to add a subpage, in the size of the other actions
  const h = page.getByTestId('hierarchy')
  await expect(h.getByRole('heading', { name: 'Hierarchy' })).toHaveCount(0)
  await expect(h.getByRole('button', { name: '+ Add subpage' })).toBeVisible()
  const size = await h
    .getByRole('button', { name: '+ Add subpage' })
    .evaluate((e) => parseFloat(getComputedStyle(e).fontSize))
  const body = await page.getByRole('main').evaluate((e) => parseFloat(getComputedStyle(e).fontSize))
  expect(size).toBeLessThan(body)

  // with a subpage it has its heading, as a small label
  backend.write('pages/Solo/Child.md', '- c\n')
  await page.reload()
  const heading = page.getByTestId('hierarchy').getByRole('heading', { name: 'Hierarchy' })
  await expect(heading).toBeVisible()
  expect(await heading.evaluate((e) => getComputedStyle(e).textTransform)).toBe('uppercase')
})
