import { expect, test } from './fixtures'

const write = (backend: { write(rel: string, text: string): void }) => {
  backend.write('pages/P/Capgemini/Renewal/Siemens Energy.md', '- TODO call Siemens\n')
  backend.write('pages/P/EY/Renewal/APotheke.md', '- TODO send offer\n- DONE kickoff\n')
  backend.write('pages/X/Renewal/Snowflake.md', '- TODO renew licence\n')
  backend.write('pages/P/EY/Other.md', '- TODO not a renewal\n')
  backend.write('pages/notes.md', '- discussed #renewal with the team, TODO follow up\n')
}

test('a segment that occurs in several places opens the view of all of them', async ({ backend, page }) => {
  write(backend)
  await backend.open('/page/P')
  const h = page.getByTestId('hierarchy')
  // `Renewal` occurs in two places below P: it is a topic, with a badge; `Capgemini` and `Siemens Energy` are not
  const topic = h.locator('a[data-segment="Renewal"]').first()
  await expect(topic.getByTestId('segment-badge')).toHaveText('3') // P/Capgemini, P/EY and X: 3 places in the whole graph (the #renewal placeholder page is not one)
  await expect(h.locator('a[data-page="P/Capgemini"]').first()).toBeVisible()
  await expect(h.locator('a[data-page="P/Capgemini"] .badge')).toHaveCount(0)
  await topic.click()
  await expect(page).toHaveURL(/#\/segment\/Renewal/)
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Renewal')
  await expect(page.getByTestId('segment-places')).toHaveText('3')
  const groups = page.getByTestId('segment-group')
  await expect(groups).toHaveCount(3)
  await expect(groups.nth(0)).toHaveAttribute('data-occurrence', 'P/Capgemini/Renewal')
  await expect(groups.nth(0)).toContainText('Siemens Energy')
  await expect(groups.nth(1)).toContainText('APotheke')
  await expect(groups.nth(2)).toContainText('Snowflake')
  await expect(groups.nth(1)).not.toContainText('Other') // P/EY/Other has no Renewal in its path
  // the open tasks of all those pages, and the explicit #renewal reference
  const tasks = page.getByTestId('page-tasks')
  await expect(tasks).toContainText('call Siemens')
  await expect(tasks).toContainText('send offer')
  await expect(tasks).toContainText('renew licence')
  await expect(tasks).not.toContainText('not a renewal')
  await expect(tasks).not.toContainText('kickoff') // done
  await expect(page.getByTestId('backlink-count')).toHaveText('1')
  await expect(page.getByRole('region', { name: 'Linked references', exact: true })).toContainText(
    'discussed',
  )
  // a page in a group opens; the group header links to the occurrence
  await groups.nth(1).locator('a[data-page="P/EY/Renewal/APotheke"]').click()
  await expect(page).toHaveURL(/P%2FEY%2FRenewal%2FAPotheke/)
})

test('on a page whose own name has a recurring segment, that segment is clickable too', async ({
  backend,
  page,
}) => {
  backend.write('pages/P/Capgemini/Renewal/Siemens Energy.md', '- TODO call Siemens\n')
  backend.write('pages/P/Infomotion/Renewal/Siemens Energy.md', '- TODO send offer\n')
  backend.write('pages/P/Capgemini/RfP/PDD/Siemens Energy.md', '- TODO draft PDD\n')
  // the page `P/Capgemini/Renewal` itself (it has no file, only pages below it)
  await backend.open('/page/P%2FCapgemini%2FRenewal')
  const h = page.getByTestId('hierarchy')
  // in its own Hierarchy `Renewal` is a topic (2 places), `Siemens Energy` too (3 places); `P` is a plain page link
  await expect(h.locator('a[data-segment="Renewal"] .badge')).toHaveText('2')
  await expect(h.locator('a[data-segment="Siemens Energy"] .badge')).toHaveText('3')
  await expect(h.locator('a[data-page="P"]')).toBeVisible()
  await expect(h.locator('a[data-page="P"] .badge')).toHaveCount(0)
  // the title says so as well, and the path above it keeps plain links for unique names
  const chip = page.getByTestId('title-topic')
  await expect(chip).toHaveText('in 2 places')
  await expect(page.getByRole('navigation', { name: 'Namespace' }).locator('a.topic')).toHaveCount(0)
  // clicking `Renewal` in the Hierarchy lists the renewals without a detour through Capgemini
  await h.locator('a[data-segment="Renewal"]').first().click()
  await expect(page).toHaveURL(/#\/segment\/Renewal/)
  const groups = page.getByTestId('segment-group')
  await expect(groups).toHaveCount(2)
  await expect(groups.nth(0)).toHaveAttribute('data-occurrence', 'P/Capgemini/Renewal')
  await expect(groups.nth(1)).toHaveAttribute('data-occurrence', 'P/Infomotion/Renewal')
  // the title chip does the same
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/P%2FInfomotion%2FRenewal')
  await page.getByTestId('title-topic').click()
  await expect(page).toHaveURL(/#\/segment\/Renewal/)
  // and in the path above the title of a deeper page, a recurring segment is a topic link
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/P%2FCapgemini%2FRenewal%2FSiemens%20Energy')
  const crumbs = page.getByRole('navigation', { name: 'Namespace' })
  await expect(crumbs.locator('a.topic[data-segment="Renewal"] .badge')).toHaveText('2')
  await expect(crumbs.locator('a[href$="P%2FCapgemini"]')).toBeVisible() // unique: a plain link
})

test('path: works in query blocks and a unique segment links to its page', async ({ backend, page }) => {
  write(backend)
  backend.write('pages/q.md', '- ```query\n  task: todo\n  path: Renewal\n  view: list\n  ```\n')
  await backend.open('/page/q')
  const list = page.getByTestId('query-list')
  await expect(list).toContainText('Siemens Energy')
  await expect(list).toContainText('Snowflake')
  await expect(list).not.toContainText('not a renewal')
  // a unique segment (`APotheke` below P/EY) is a plain page link without a badge
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/P%2FEY')
  const h = page.getByTestId('hierarchy')
  await expect(h.locator('a[data-page="P/EY/Renewal/APotheke"]')).toBeVisible()
  await expect(h.locator('a[data-page="P/EY/Renewal/APotheke"] .badge')).toHaveCount(0)
})

test('a linked-only page inside a path counts as a place: `deepest` in two paths gets its badge', async ({
  backend,
  page,
}) => {
  // as in a real graph: `Napis/first/deepest` is only linked (it shows in italics), the other one is a file
  backend.write('pages/Napis.md', '- see [[Napis/first/deepest]]\n')
  backend.write('pages/Napis/first.md', '- first\n')
  backend.write('pages/Napis/second.md', '- second\n')
  backend.write('pages/Napis/second/deeper/deepest.md', '- the real one\n')
  await backend.open('/page/Napis')
  const h = page.getByTestId('hierarchy')
  await expect(h.locator('li[data-page="Napis/first/deepest"]')).toBeVisible()
  const badge = h.locator('a[data-segment="deepest"]').first().getByTestId('segment-badge')
  await expect(badge).toHaveText('2')
  await h.locator('a[data-segment="deepest"]').first().click()
  await expect(page).toHaveURL(/#\/segment\/deepest/)
  const groups = page.getByTestId('segment-group')
  await expect(groups).toHaveCount(2)
  await expect(groups.nth(0)).toHaveAttribute('data-occurrence', 'Napis/first/deepest')
  await expect(groups.nth(1)).toHaveAttribute('data-occurrence', 'Napis/second/deeper/deepest')
})
