import { expect, test } from './fixtures'

// Aliases (LN-6): other names for a page, added in its properties, usable in `[[links]]`.
test('an alias is added in the properties, a link through it opens the page, and it can be removed', async ({
  backend,
  page,
}) => {
  backend.write('pages/Target.md', '- the target\n')
  backend.write('pages/Linker.md', '- see [[tgt]]\n')
  await backend.open('/page/Target')
  const main = page.getByRole('main')
  await main.getByTestId('props-toggle').click()
  await expect(main.getByTestId('alias')).toHaveCount(0)
  await main.getByTestId('alias-input').fill('tgt, The Target')
  await main.getByTestId('alias-input').press('Enter')
  await expect(main.getByTestId('alias')).toHaveText(['tgt', 'The Target'])
  await expect.poll(() => backend.read('pages/Target.md')).toContain('aliases: [tgt, The Target]')
  expect(backend.read('pages/Target.md')).toContain('- the target')

  // the link through the alias opens the page, and the page lists the link as a reference
  await expect(main.getByText('Linker')).toBeVisible({ timeout: 10000 }) // in the linked references
  await backend.open('/page/Linker')
  await main.getByRole('link', { name: 'tgt' }).click()
  await expect(page).toHaveURL(/#\/page\/Target$|#\/page\/tgt$/)
  await expect(main.getByText('the target')).toBeVisible()

  // removed with its ×
  await main.getByTestId('props-toggle').click()
  await main.getByRole('button', { name: 'Remove the alias tgt' }).click()
  await expect(main.getByTestId('alias')).toHaveText(['The Target'])
  await main.getByRole('button', { name: 'Remove the alias The Target' }).click()
  await expect.poll(() => backend.read('pages/Target.md')).not.toContain('aliases')
})

test('an alias that another page has is refused with a message', async ({ backend, page }) => {
  backend.write('pages/Target.md', '---\naliases: [tgt]\n---\n- a\n')
  backend.write('pages/Other.md', '- b\n')
  await backend.open('/page/Other')
  const main = page.getByRole('main')
  await main.getByTestId('props-toggle').click()
  await main.getByTestId('alias-input').fill('tgt')
  await main.getByTestId('alias-input').press('Enter')
  await expect(main.getByTestId('alias-error')).toContainText('Target')
  await expect(main.getByTestId('alias')).toHaveCount(0)
  expect(backend.read('pages/Other.md')).toBe('- b\n')
})
