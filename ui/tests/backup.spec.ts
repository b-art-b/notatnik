import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from './fixtures'

// BK-1..BK-5: the Backup tab of the settings.
test('backup: set the folder, back up now, restore one page and a whole folder', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- original\n')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Backup/ }).click()
  const pane = page.getByRole('tabpanel')
  await expect(pane.getByRole('heading', { name: 'Backup' })).toBeVisible()
  await expect(pane.getByRole('switch', { name: 'Automatic backups' })).toBeChecked()
  await expect(pane.getByTestId('backup-item')).toHaveCount(0)
  // the way back is explained on the page
  await expect(pane.getByRole('heading', { name: 'How to restore' })).toBeVisible()
  await expect(pane.getByText('Restore folder', { exact: false }).first()).toBeVisible()

  // the folder is configurable; a path inside the graph is refused
  const dir = path.join(backend.dir, '..', 'my-backups')
  const folder = pane.getByLabel('Backup folder')
  await folder.fill(path.join(backend.dir, 'inside'))
  await folder.press('Enter')
  await expect(page.getByRole('alert').filter({ hasText: /outside the graph folder/ })).toBeVisible()
  await folder.fill(dir)
  await folder.press('Enter')
  await expect(pane.getByRole('button', { name: 'Use default' })).toBeEnabled()

  await pane.getByTestId('backup-now').click()
  const items = pane.getByTestId('backup-item')
  await expect(items).toHaveCount(1)
  expect(fs.readdirSync(dir).filter((f) => f.endsWith('.zip'))).toHaveLength(1)

  // damage the page on disk, bring it back from the snapshot
  fs.writeFileSync(path.join(backend.dir, 'pages/t.md'), '- ruined\n')
  await items.first().getByRole('button', { name: 'Restore…' }).click()
  await items.first().getByLabel('Page name').fill('t')
  await items.first().getByRole('button', { name: 'Restore page' }).click()
  await expect.poll(() => backend.read('pages/t.md')).toBe('- original\n')

  // the snapshot taken before the page was replaced is marked; the one made by hand is not
  const scheduled = items.filter({ hasNotText: 'before a change' })
  await expect(items.filter({ hasText: 'before a change' })).toHaveCount(1)
  // everything into a new folder, from the original snapshot (the newest one holds the ruined page)
  await expect(items).toHaveCount(2)
  await scheduled.getByRole('button', { name: 'Restore…' }).click()
  const target = path.join(backend.dir, '..', 'restored')
  await scheduled.getByLabel('Target folder').fill(target)
  await scheduled.getByRole('button', { name: 'Restore folder' }).click()
  await expect.poll(() => fs.existsSync(path.join(target, 'pages/t.md'))).toBe(true)
  expect(fs.readFileSync(path.join(target, 'pages/t.md'), 'utf8')).toBe('- original\n')

  // the automatic switch is a device setting
  await pane.getByRole('switch', { name: 'Automatic backups' }).uncheck()
  await page.reload()
  await page.getByRole('tab', { name: /Backup/ }).click()
  await expect(
    page.getByRole('tabpanel').getByRole('switch', { name: 'Automatic backups' }),
  ).not.toBeChecked()
})
