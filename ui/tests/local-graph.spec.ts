import { expect, test } from './fixtures'

// LN-9: the local graph, a tab of the side panel. LN-13: the pages with no links.
const palette = async (page: import('@playwright/test').Page, command: string) => {
  await page.keyboard.press('Control+k')
  const dlg = page.getByRole('dialog', { name: 'Command palette' })
  await dlg.getByLabel('Command or page').fill(command)
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(dlg).toHaveCount(0)
}

const write = (backend: { write: (p: string, t: string) => void }) => {
  backend.write('pages/Hub.md', '- to [[Out1]] and #Out2\n')
  backend.write('pages/In1.md', '- see [[Hub]]\n')
  backend.write('pages/Out1.md', '- and [[Far]] and [[In1]]\n')
  backend.write('pages/Out2.md', '- x\n')
  backend.write('pages/Far.md', '- end\n')
}

test('Local graph opens a tab beside the page; a node opens its page and the graph follows', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  write(backend)
  await backend.open('/page/Hub')
  await palette(page, 'Local graph')
  const side = page.getByTestId('side-panel')
  await expect(side.getByRole('region', { name: 'Local graph' })).toBeVisible()
  const picture = side.getByRole('group', { name: 'Pages around Hub' })
  await expect(picture.getByRole('link')).toHaveCount(4) // Hub, In1, Out1, Out2
  for (const n of ['Hub', 'In1', 'Out1', 'Out2'])
    await expect(picture.getByRole('link', { name: n })).toBeVisible()
  // the same pages as text
  await expect(side.getByRole('heading', { name: 'Links to' })).toBeVisible()
  await expect(side.getByRole('heading', { name: 'Linked from' })).toBeVisible()
  // a node by keyboard: Enter on the focused link opens the page, and the graph is drawn around it
  await picture.getByRole('link', { name: 'Out1' }).focus()
  await page.keyboard.press('Enter')
  await expect(page).toHaveURL(/#\/page\/Out1/)
  await expect(side.getByRole('group', { name: 'Pages around Out1' })).toBeVisible()
  // one step from Out1: Hub, In1 and Far
  await expect(side.getByRole('group', { name: 'Pages around Out1' }).getByRole('link')).toHaveCount(4)
})

test('1 step and 2 steps; the namespace field leaves pages out but not the page itself', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  write(backend)
  backend.write('pages/work/Plan.md', '- [[Hub]]\n')
  await backend.open('/page/Hub')
  await palette(page, 'Local graph')
  const side = page.getByTestId('side-panel')
  const picture = side.getByRole('group', { name: 'Pages around Hub' })
  await expect(picture.getByRole('link', { name: 'Far' })).toHaveCount(0)
  await side.getByRole('button', { name: '2 steps' }).click()
  await expect(picture.getByRole('link', { name: 'Far' })).toBeVisible()
  await expect(side.getByRole('heading', { name: 'Further away' })).toBeVisible()
  await side.getByRole('button', { name: '1 step' }).click()
  await expect(picture.getByRole('link', { name: 'Far' })).toHaveCount(0)
  // the namespace field: only work/…, and Hub stays
  await side.getByLabel('Only pages in the namespace').fill('work')
  await expect(picture.getByRole('link', { name: 'work/Plan' })).toBeVisible()
  await expect(picture.getByRole('link', { name: 'Hub' })).toBeVisible()
  await expect(picture.getByRole('link', { name: 'In1' })).toHaveCount(0)
})

test('a page with no links says so; × closes the tab and the other tabs go on working', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  backend.write('pages/Lonely.md', '- nothing\n')
  await backend.open('/page/Lonely')
  await palette(page, 'Local graph')
  const side = page.getByTestId('side-panel')
  await expect(side.getByText('This page has no links to or from other pages.')).toBeVisible()
  await expect(side.getByRole('group', { name: 'Pages around Lonely' }).getByRole('link')).toHaveCount(1)
  await side.getByTestId('graph-tab-close').click()
  await expect(side.getByRole('group', { name: /Pages around/ })).toHaveCount(0)
})

test('a long name is shortened in the picture and whole in its title and the list', async ({
  backend,
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 })
  const long = 'An extremely long page name that does not fit'
  backend.write('pages/Hub.md', `- [[${long}]]\n`)
  backend.write(`pages/${long}.md`, '- x\n')
  await backend.open('/page/Hub')
  await palette(page, 'Local graph')
  const side = page.getByTestId('side-panel')
  const node = side.getByRole('group', { name: 'Pages around Hub' }).getByRole('link', { name: long })
  await expect(node.locator('text')).toHaveText(/…$/)
  await expect(node.locator('title')).toHaveText(long)
  await expect(side.getByRole('list').getByRole('link', { name: long })).toBeVisible()
})

test('Pages with no links lists the unlinked pages only; linking one makes it leave', async ({
  backend,
  page,
}) => {
  backend.write('pages/Island.md', '- nothing here\n')
  backend.write('pages/A.md', '- see [[B]]\n')
  backend.write('pages/B.md', '- b\n')
  backend.write('pages/Archived island.md', '---\narchived: true\n---\n- hidden\n')
  await backend.open('/journals')
  await palette(page, 'Pages with no links')
  await expect(page).toHaveURL(/#\/pages\?links=none/)
  await expect(page.getByRole('heading', { name: /Pages with no links/ })).toBeVisible()
  const list = page.getByRole('main')
  await expect(list.getByRole('link', { name: 'Island', exact: true })).toBeVisible()
  await expect(list.getByRole('link', { name: 'A', exact: true })).toHaveCount(0)
  await expect(list.getByRole('link', { name: 'B', exact: true })).toHaveCount(0)
  await expect(list.getByRole('link', { name: 'Archived island' })).toHaveCount(0)
  // the checkbox leaves the filter
  await list.getByLabel('Only pages with no links').uncheck()
  await expect(page).toHaveURL(/#\/pages$/)
  await expect(list.getByRole('link', { name: 'B', exact: true })).toBeVisible()
  await list.getByLabel('Only pages with no links').check()
  // linking Island from A: it leaves the list
  backend.write('pages/A.md', '- see [[B]] and [[Island]]\n')
  await page.reload()
  await expect(page.getByRole('heading', { name: /Pages with no links/ })).toBeVisible()
  await expect(page.getByRole('main').getByRole('link', { name: 'Island', exact: true })).toHaveCount(0)
})
