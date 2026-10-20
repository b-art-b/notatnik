import { expect, test } from './fixtures'

// The list of settings sections: grouped, searchable, one tablist (arrow keys), every section starts alike.
test('settings sections are grouped, and the search narrows them and opens the first match', async ({
  backend,
  page,
}) => {
  await backend.open('/settings')
  const tabs = page.getByRole('tablist', { name: 'Settings categories' })
  // the groups name what the sections are about
  for (const g of ['Look and feel', 'Notes', 'Servers', 'Data']) {
    await expect(tabs.getByText(g, { exact: true })).toBeVisible()
  }
  await expect(tabs.getByRole('tab')).toHaveCount(10)
  // every section opens with its title and where it is kept
  await expect(page.getByRole('heading', { level: 2, name: 'Appearance' })).toBeVisible()
  await expect(page.getByTestId('settings-scope')).toHaveText('This device')

  // a word of a setting finds the sections that hold it, and the first match opens
  const search = page.getByRole('searchbox', { name: 'Search settings' })
  await search.fill('encrypt')
  await expect(tabs.getByRole('tab')).toHaveText(['Sync', 'Backup'])
  await expect(page.getByRole('heading', { level: 2, name: 'Sync' })).toBeVisible()
  await expect(page.getByTestId('settings-scope')).toHaveText('This graph')
  // narrowing further keeps the open section when it still matches
  await search.fill('encrypt snapshot')
  await expect(tabs.getByRole('tab')).toHaveText(['Backup'])
  await expect(page.getByRole('heading', { level: 2, name: 'Backup' })).toBeVisible()
  // nothing matches: say so, and Escape clears the search
  await search.fill('zzzz')
  await expect(page.getByText('No setting matches')).toBeVisible()
  await search.press('Escape')
  await expect(search).toHaveValue('')
  await expect(tabs.getByRole('tab')).toHaveCount(10)
})

test('arrow keys move between the sections of the list', async ({ backend, page }) => {
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Appearance/ }).focus()
  await page.keyboard.press('ArrowDown')
  await expect(page.getByRole('tab', { name: /Dates/ })).toBeFocused()
  await expect(page.getByRole('heading', { level: 2, name: 'Dates & language' })).toBeVisible()
  await page.keyboard.press('End')
  await expect(page.getByRole('tab', { name: /Graph/ })).toBeFocused()
  await page.keyboard.press('ArrowDown') // wraps around to the first
  await expect(page.getByRole('tab', { name: /Appearance/ })).toBeFocused()
})

// Backup is split in three parts under its title; the part that is open is marked for assistive technology.
test('backup has three parts, one open at a time', async ({ backend, page }) => {
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Backup/ }).click()
  const parts = page.getByRole('navigation', { name: 'Backup sections' })
  await expect(parts.getByRole('button')).toHaveText(['Snapshots', 'Where & when', 'Encryption'])
  await expect(parts.getByRole('button', { name: 'Snapshots' })).toHaveAttribute('aria-current', 'page')
  await expect(page.getByTestId('backup-now')).toBeVisible()
  await expect(page.getByLabel('Backup folder')).toHaveCount(0)

  await parts.getByRole('button', { name: 'Where & when' }).click()
  await expect(page.getByLabel('Backup folder')).toBeVisible()
  await expect(page.getByTestId('backup-now')).toHaveCount(0)
  await expect(parts.getByRole('button', { name: 'Where & when' })).toHaveAttribute('aria-current', 'page')

  await parts.getByRole('button', { name: 'Encryption' }).click()
  await expect(page.getByTestId('enc-setup')).toBeVisible()
  await expect(page.getByLabel('Backup folder')).toHaveCount(0)
  // the explanation is folded away
  await parts.getByRole('button', { name: 'Snapshots' }).click()
  await expect(page.getByText('Restore folder', { exact: false }).first()).toBeHidden()
  await page.getByText('How to restore', { exact: true }).click()
  await expect(page.getByText('Restore folder', { exact: false }).first()).toBeVisible()
})
