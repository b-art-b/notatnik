import { expect, test } from './fixtures'

test('page icons: a rule in Settings puts the emoji next to the pages everywhere', async ({
  backend,
  page,
}) => {
  backend.write('pages/Ann.md', '---\ntype: person\n---\n- ann\n')
  backend.write('pages/Bob.md', '---\ntype: Person\nicon: 🧑‍🔧\n---\n- bob\n')
  backend.write('pages/Acme.md', '---\ntype: client\n---\n- acme, contact [[Ann]]\n')
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Page icons' }).click()
  await expect(page.getByText('No rules yet.')).toBeVisible()
  await page.getByRole('button', { name: 'Add a rule' }).click()
  const rule = page.getByTestId('icon-rule').first()
  await rule.getByLabel('Property').fill('type')
  await rule.getByLabel('Value').fill('person')
  await rule.getByLabel('Value').blur()
  // not saved yet: the icon is missing
  await expect.poll(() => backend.read('.notatnik/config.toml')).not.toContain('page_icons')
  await rule.getByRole('button', { name: 'Choose an icon' }).click()
  await page.getByLabel('Search emojis').fill('bust')
  await page.getByRole('option', { name: 'bust in silhouette' }).click()
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('icon = "👤"')
  expect(backend.read('.notatnik/config.toml')).toContain('value = "person"')
  // a second rule for the clients, typed instead of picked
  await page.getByRole('button', { name: 'Add a rule' }).click()
  const second = page.getByTestId('icon-rule').nth(1)
  await second.getByLabel('Value').fill('client')
  await second.getByRole('button', { name: 'Choose an icon' }).click()
  await page.getByLabel('Search emojis').fill('office')
  await page.keyboard.press('Enter') // the first match
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('value = "client"')
  // the icons show up: page title, links in text, the sidebar list grouped by type
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/Ann')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('👤')
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/Acme')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('🏢')
  await expect(page.locator('a.page-ref[data-page="Ann"]')).toContainText('👤')
  await page.getByLabel('Group pages by').selectOption('type')
  await page.getByRole('button', { name: 'Expand all' }).click()
  await expect(page.locator('[data-testid="grouped-pages"] a[data-page="Ann"]')).toContainText('👤')
  // Bob's own `icon:` wins over the `type: Person` rule
  await expect(page.locator('[data-testid="grouped-pages"] a[data-page="Bob"]')).toContainText('🧑‍🔧')
  await expect(page.locator('[data-testid="grouped-pages"] a[data-page="Bob"]')).not.toContainText('👤')
  // removing the rules takes the icons away again (Bob keeps his own)
  await page.goto(page.url().replace(/#.*$/, '') + '#/settings')
  await page.getByRole('tab', { name: 'Page icons' }).click()
  await page.getByRole('button', { name: 'Remove rule 2' }).click()
  await page.getByRole('button', { name: 'Remove rule 1' }).click()
  await expect.poll(() => backend.read('.notatnik/config.toml')).not.toContain('page_icons')
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/Ann')
  await expect(page.getByRole('heading', { level: 1 })).not.toContainText('👤')
})

test('typing :shortcode in a note suggests emojis and inserts the character', async ({ backend, page }) => {
  backend.write('pages/e.md', '- launch\n')
  await backend.open('/page/e')
  await page.getByText('launch').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' :rocke')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('rocket')
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('block-editor')).toHaveText('launch 🚀')
  // `12:30` and `key::` are not shortcodes
  await page.keyboard.type(' at 12:30')
  await page.waitForTimeout(300)
  await expect(page.locator('.cm-tooltip-autocomplete')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/e.md')).toBe('- launch 🚀 at 12:30\n')
})

test('/emoji opens a picker under the cursor; search, Enter and Escape', async ({ backend, page }) => {
  backend.write('pages/e.md', '- ship it\n')
  await backend.open('/page/e')
  await page.getByText('ship it').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('End')
  await page.keyboard.type(' /emoji')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Emoji')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  const picker = page.getByTestId('emoji-picker')
  await expect(picker).toBeVisible()
  await page.keyboard.type('tada')
  await page.keyboard.press('Enter') // the first match
  await expect(picker).toBeHidden()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await expect(page.getByTestId('block-editor')).toHaveText('ship it 🎉')
  // Escape closes the picker and keeps the text
  await page.keyboard.type(' /emoji')
  await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('Emoji')
  await page.waitForTimeout(150)
  await page.keyboard.press('Enter')
  await expect(picker).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(picker).toBeHidden()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect.poll(() => backend.read('pages/e.md')).toBe('- ship it 🎉\n')
})

test('journal days get an icon rule like any page; the calendar stays until one is set', async ({
  backend,
  page,
}) => {
  backend.write('journals/2026/2026-10-10.md', '- a day\n')
  await backend.open('/journals')
  // no rule yet: the drawn calendar in front of the day's title
  const feed = page.getByRole('main')
  await expect(feed.locator('.jicon').first()).toBeVisible()
  await backend.open('/settings')
  await page.getByRole('tab', { name: 'Page icons' }).click()
  await page.getByTestId('add-journal-rule').click()
  const rule = page.getByTestId('icon-rule').first()
  await expect(rule.getByLabel('Property')).toHaveValue('type')
  await expect(rule.getByLabel('Value')).toHaveValue('journal')
  await page.getByLabel('Search emojis').fill('spiral calendar')
  await page.keyboard.press('Enter')
  await expect.poll(() => backend.read('.notatnik/config.toml')).toContain('value = "journal"')
  await page.goto(page.url().replace(/#.*$/, '') + '#/journals')
  await expect(page.getByRole('main').getByTestId('page-icon').first()).toBeVisible()
  await expect(page.getByRole('main').locator('.jicon')).toHaveCount(0)
  // the day's own page lists the default type in its properties
  await page.goto(page.url().replace(/#.*$/, '') + '#/page/2026-10-10')
  await page.getByTestId('props-toggle').click()
  await expect(page.getByTestId('default-type')).toContainText('journal')
})

test('page icons are quiet in lists and full color in titles; the style is a device setting', async ({
  backend,
  page,
}) => {
  backend.write('pages/Ann.md', '---\nicon: 👤\n---\n- ann\n')
  backend.write('pages/Plain.md', '- plain\n')
  await backend.open('/pages')
  const table = page.getByTestId('pages-table')
  const row = (name: string) => table.locator('tr', { has: page.locator(`a[data-page="${name}"]`) })
  const icon = row('Ann').getByTestId('page-icon')
  const style = (el: typeof icon) =>
    el.evaluate((e) => {
      const cs = getComputedStyle(e)
      return {
        opacity: cs.opacity,
        filter: cs.filter,
        size: parseFloat(cs.fontSize),
        width: e.getBoundingClientRect().width,
      }
    })

  // muted is the default: smaller than the text, grey and faint in a list, in a box that is a little wider than the glyph
  await expect(icon).toBeVisible()
  const rowSize = await row('Ann')
    .locator('a[data-page]')
    .evaluate((e) => parseFloat(getComputedStyle(e).fontSize))
  await expect.poll(async () => (await style(icon)).opacity).toBe('0.72')
  const muted = await style(icon)
  expect(muted.filter).toContain('grayscale')
  expect(muted.size).toBeLessThan(rowSize)
  expect(muted.width).toBeCloseTo(muted.size * 1.3, 0)
  // a page without an icon keeps the same room, so the names line up
  const blank = await row('Plain')
    .locator('.picon.blank')
    .evaluate((e) => e.getBoundingClientRect().width)
  expect(blank).toBeCloseTo(muted.width, 0)
  // full color where the eye is: under the pointer
  await row('Ann').locator('a[data-page]').hover()
  await expect.poll(async () => (await style(icon)).opacity).toBe('1')
  expect((await style(icon)).filter).toBe('none')

  // and in the title of the page
  await backend.open('/page/Ann')
  const title = page.getByRole('main').locator('h1').getByTestId('page-icon')
  await expect(title).toBeVisible()
  expect(await style(title)).toMatchObject({ opacity: '1', filter: 'none' })
  const h1 = await page
    .getByRole('main')
    .locator('h1')
    .evaluate((e) => parseFloat(getComputedStyle(e).fontSize))
  expect((await style(title)).size).toBeLessThan(h1)

  // the setting: Color leaves the emoji alone, Grey keeps it grey also under the pointer
  await backend.open('/settings')
  await page
    .getByRole('radiogroup', { name: 'Page icon style' })
    .getByRole('radio', { name: 'Color' })
    .check()
  await expect
    .poll(async () => (await backend.call<{ icon_style: string }>('get_settings')).icon_style)
    .toBe('color')
  await backend.open('/pages')
  await expect.poll(async () => (await style(icon)).filter).toBe('none')
  expect((await style(icon)).opacity).toBe('1')
  await backend.call('set_settings', { icon_style: 'mono' })
  await page.reload() // the app reads the device settings when it starts
  await row('Ann').locator('a[data-page]').hover()
  await expect.poll(async () => (await style(icon)).filter).toContain('grayscale(1)')
})
