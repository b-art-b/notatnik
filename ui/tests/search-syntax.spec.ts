import { expect, test } from './fixtures'

// SR-4: AND, OR, -word, parentheses, phrases and prefixes in the search box.
test('the search understands OR, minus, parentheses and phrases, and explains a mistake', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/notes.md',
    '- budget report draft\n- budget forecast final\n- annual plan report\n- unrelated words\n',
  )
  await backend.open('/journals')
  await page.keyboard.press('Control+Shift+f')
  const field = page.getByLabel('Search text')
  const hits = page.getByTestId('search-hit')
  const blockTexts = async () =>
    (await hits.filter({ hasText: 'block' }).allInnerTexts())
      .map((t) => t.replace(/^block\s*notes\s*/, '').trim())
      .sort()
  await expect(page.getByTestId('search-syntax')).toContainText('OR')
  await field.fill('budget report')
  await expect.poll(blockTexts).toEqual(['budget report draft'])
  await field.fill('forecast OR annual')
  await expect.poll(blockTexts).toEqual(['annual plan report', 'budget forecast final'])
  await field.fill('report -draft')
  await expect.poll(blockTexts).toEqual(['annual plan report'])
  await field.fill('(budget OR annual) report')
  await expect.poll(blockTexts).toEqual(['annual plan report', 'budget report draft'])
  await field.fill('"annual plan"')
  await expect.poll(blockTexts).toEqual(['annual plan report'])
  // the matching words are marked, the excluded ones are not searched for
  await field.fill('budget -report')
  await expect.poll(async () => (await hits.locator('mark').allInnerTexts()).join(',')).toBe('budget')
  // half-typed syntax is explained and the results stay
  await field.fill('(budget')
  await expect(page.getByRole('alert')).toContainText('not closed')
  await field.fill('(budget)')
  await expect(page.getByRole('alert')).toHaveCount(0)
  await field.fill('-budget')
  await expect(page.getByRole('alert')).toContainText('takes results away')
})
