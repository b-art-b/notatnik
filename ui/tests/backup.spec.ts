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

// BK-7..BK-9: compression, the second folder, and the device settings inside a snapshot.
test('backup: compression choice, second folder, and restoring the device settings with a preview', async ({
  backend,
  page,
}) => {
  backend.write('pages/t.md', '- original\n')
  const first = path.join(backend.dir, '..', 'first')
  const second = path.join(backend.dir, '..', 'second')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Backup/ }).click()
  const pane = page.getByRole('tabpanel')
  await pane.getByLabel('Backup folder').fill(first)
  await pane.getByLabel('Backup folder').press('Enter')

  // the compression choice is saved and used for new snapshots
  await expect(pane.getByRole('radio', { name: 'Compressed', exact: true })).toBeChecked()
  await pane.getByRole('radio', { name: 'Not compressed' }).check()
  await pane.getByTestId('backup-now').click()
  await expect(pane.getByTestId('backup-item')).toHaveCount(1)
  const zip = fs.readdirSync(first).find((f) => f.endsWith('.zip'))!
  // a stored entry: the text of the page is readable inside the zip file
  expect(fs.readFileSync(path.join(first, zip)).includes('- original')).toBe(true)

  // second folder: a folder that is not there is reported, one that exists gets the copies
  const mirror = pane.getByLabel('Second folder')
  await mirror.fill(second)
  await mirror.press('Enter')
  await pane.getByTestId('backup-now').click()
  await expect(pane.getByTestId('mirror-error')).toContainText('not available')
  fs.mkdirSync(second)
  await pane.getByTestId('backup-now').click()
  await expect(pane.getByTestId('mirror-error')).toHaveCount(0)
  await expect(pane.getByTestId('backup-item').first()).toContainText('copied')
  expect(fs.readdirSync(second).filter((f) => f.endsWith('.zip')).length).toBeGreaterThan(0)
  // the same folder twice is refused
  await mirror.fill(first)
  await mirror.press('Enter')
  await expect(page.getByRole('alert').filter({ hasText: /must differ/ })).toBeVisible()
  await mirror.fill(second)

  // device settings: change one after the snapshot, compare, restore
  await page.getByRole('tab', { name: /Appearance/ }).click()
  await page.getByRole('radio', { name: 'Dark', exact: true }).check()
  await page.getByRole('tab', { name: /Backup/ }).click()
  const item = pane.getByTestId('backup-item').last()
  await item.getByRole('button', { name: 'Restore…' }).click()
  await item.getByRole('button', { name: 'Compare…' }).click()
  const diff = item.getByTestId('device-diff')
  await expect(diff).toContainText('theme = "dark"')
  await item.getByRole('button', { name: 'Restore settings' }).click()
  await page.getByRole('tab', { name: /Appearance/ }).click()
  await expect(page.getByRole('radio', { name: 'System', exact: true })).toBeChecked()
})

// BK-10: before restoring, every file that differs is listed, and its lines can be opened.
test('backup: compare a snapshot with the current state before restoring', async ({ backend, page }) => {
  backend.write('pages/t.md', '- original\n- same line\n')
  backend.write('pages/gone.md', '- will be deleted\n')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Backup/ }).click()
  const pane = page.getByRole('tabpanel')
  await pane.getByLabel('Backup folder').fill(path.join(backend.dir, '..', 'cmp-backups'))
  await pane.getByLabel('Backup folder').press('Enter')
  await pane.getByTestId('backup-now').click()
  const item = pane.getByTestId('backup-item').first()
  await expect(item).toBeVisible()

  // nothing differs yet
  await item.getByRole('button', { name: 'Restore…' }).click()
  await expect(item.getByTestId('backup-changes')).toContainText('Nothing differs')
  await item.getByRole('button', { name: 'Restore…' }).click()

  // change a page, delete one, add one; the list shows all three
  fs.writeFileSync(path.join(backend.dir, 'pages/t.md'), '- changed\n- same line\n')
  fs.rmSync(path.join(backend.dir, 'pages/gone.md'))
  fs.writeFileSync(path.join(backend.dir, 'pages/new.md'), '- brand new\n')
  await item.getByRole('button', { name: 'Restore…' }).click()
  const changes = item.getByTestId('backup-changes')
  await expect(changes.getByRole('button', { name: /pages\/t\.md/ })).toBeVisible()
  await expect(changes.getByRole('button', { name: /pages\/gone\.md/ })).toContainText('deleted since')
  await expect(changes.getByRole('button', { name: /pages\/new\.md/ })).toContainText('new since')

  // open one file: the line now and the line in the snapshot, with the unchanged line as context
  await changes.getByRole('button', { name: /pages\/t\.md/ }).click()
  const diff = changes.getByLabel('Changes in pages/t.md')
  await expect(diff).toContainText('− - changed')
  await expect(diff).toContainText('+ - original')
  await expect(diff).toContainText('- same line')
  // all at once
  await changes.getByRole('button', { name: 'Show all changes' }).click()
  await expect(changes.getByLabel('Changes in pages/gone.md')).toContainText('+ - will be deleted')
  await changes.getByRole('button', { name: 'Hide all' }).click()
  await expect(changes.getByLabel('Changes in pages/t.md')).toHaveCount(0)
})

// EN-2, EN-5, EN-6: set up encryption, back up, lock, unlock with the password and with the recovery key.
test('backup encryption: set up with a recovery key, seal, lock and unlock', async ({ backend, page }) => {
  backend.write('pages/t.md', '- NARWHAL-MARKER-7 in a page\n')
  const dir = path.join(backend.dir, '..', 'sealed-backups')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Backup/ }).click()
  const pane = page.getByRole('tabpanel')
  await pane.getByLabel('Backup folder').fill(dir)
  await pane.getByLabel('Backup folder').press('Enter')

  // set up: two passwords that match, then the recovery key is shown once and must be confirmed
  await pane.getByTestId('enc-setup').click()
  const form = pane.locator('form.encform')
  await form.getByLabel('Password (8 to 200 characters)').fill('correct horse')
  await form.getByLabel('Password again').fill('something else')
  await expect(form.getByRole('button', { name: 'Create' })).toBeDisabled()
  await form.getByLabel('Password again').fill('correct horse')
  await form.getByRole('button', { name: 'Create' }).click()
  const key = (await pane.getByTestId('recovery-key').textContent())!.trim()
  expect(key.split('-')).toHaveLength(14)
  const turnOn = pane.getByTestId('enc-confirm')
  await expect(turnOn).toBeDisabled()
  await pane.getByLabel('I saved the recovery key').check()
  await turnOn.click()
  await expect(pane.getByRole('switch', { name: 'Encrypt new backups' })).toBeChecked()
  await expect(pane.getByTestId('enc-state')).toHaveText('Unlocked')
  await expect(pane.getByTestId('recovery-box')).toHaveCount(0)

  // a backup is sealed: a .zip.enc file, a lock badge, nothing readable in the folder
  await pane.getByTestId('backup-now').click()
  const item = pane.getByTestId('backup-item').first()
  await expect(item).toContainText('encrypted')
  const files = fs.readdirSync(dir)
  expect(files.some((f) => f.endsWith('.zip.enc'))).toBe(true)
  for (const f of files) {
    const bytes = fs.readFileSync(path.join(dir, f))
    expect(bytes.includes('NARWHAL-MARKER')).toBe(false)
  }

  // locked: opening the snapshot asks for the password; a wrong one is refused
  await pane.getByRole('button', { name: 'Lock', exact: true }).click()
  await expect(pane.getByTestId('enc-state')).toHaveText('Locked')
  await item.getByRole('button', { name: 'Restore…' }).click()
  const ask = item.getByLabel('Password or recovery key')
  await ask.fill('wrong password')
  await item.getByRole('button', { name: 'Unlock' }).click()
  await expect(page.getByRole('alert').filter({ hasText: /wrong password or recovery key/ })).toBeVisible()
  await ask.fill('correct horse')
  await item.getByRole('button', { name: 'Unlock' }).click()
  await expect(item.getByTestId('backup-changes')).toContainText('Nothing differs')

  // the recovery key opens it as well
  await pane.getByRole('button', { name: 'Lock', exact: true }).click()
  await pane.getByRole('button', { name: 'Unlock…' }).click()
  // (the open snapshot has an unlock form of its own: this one is the form with a Cancel button)
  const top = pane.locator('form.encform').filter({ has: page.getByRole('button', { name: 'Cancel' }) })
  await top.getByLabel('Password or recovery key').fill(key)
  await top.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(pane.getByTestId('enc-state')).toHaveText('Unlocked')

  // the choice survives a reload; the vault and the state are on the device
  await page.reload()
  await page.getByRole('tab', { name: /Backup/ }).click()
  await expect(page.getByRole('tabpanel').getByRole('switch', { name: 'Encrypt new backups' })).toBeChecked()
})

// EN-22: the password and the recovery key are lost: a new vault replaces the old one.
test('backup encryption: a forgotten password is reset', async ({ backend, page }) => {
  backend.write('pages/t.md', '- a page\n')
  const dir = path.join(backend.dir, '..', 'sealed-backups-reset')
  await backend.open('/settings')
  await page.getByRole('tab', { name: /Backup/ }).click()
  const pane = page.getByRole('tabpanel')
  await pane.getByLabel('Backup folder').fill(dir)
  await pane.getByLabel('Backup folder').press('Enter')
  await pane.getByTestId('enc-setup').click()
  const form = pane.locator('form.encform')
  await form.getByLabel('Password (8 to 200 characters)').fill('old password')
  await form.getByLabel('Password again').fill('old password')
  await form.getByRole('button', { name: 'Create' }).click()
  await pane.getByLabel('I saved the recovery key').check()
  await pane.getByTestId('enc-confirm').click()
  await expect(pane.getByTestId('enc-state')).toHaveText('Unlocked')
  await pane.getByRole('button', { name: 'Lock', exact: true }).click()

  // the reset needs the checkbox, and the new recovery key has to be confirmed like the first one
  await pane.getByTestId('enc-forgot').click()
  const reset = pane.getByTestId('enc-reset-form')
  await reset.getByLabel('New password (8 to 200 characters)').fill('new password')
  await reset.getByLabel('New password again').fill('new password')
  await expect(reset.getByRole('button', { name: 'Reset the password' })).toBeDisabled()
  await pane.getByTestId('enc-reset-understood').check()
  await reset.getByRole('button', { name: 'Reset the password' }).click()
  const key = (await pane.getByTestId('recovery-key').textContent())!.trim()
  expect(key.split('-')).toHaveLength(14)
  await expect(pane.getByTestId('enc-confirm')).toHaveText('Replace the password')
  await pane.getByLabel('I saved the recovery key').check()
  await pane.getByTestId('enc-confirm').click()
  await expect(pane.getByTestId('recovery-box')).toHaveCount(0)

  // only the new password opens the vault now
  await pane.getByRole('button', { name: 'Lock', exact: true }).click()
  await pane.getByRole('button', { name: 'Unlock…' }).click()
  const unlock = pane.locator('form.encform')
  await unlock.getByLabel('Password or recovery key').fill('old password')
  await unlock.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(page.getByRole('alert').filter({ hasText: /wrong password or recovery key/ })).toBeVisible()
  await unlock.getByLabel('Password or recovery key').fill('new password')
  await unlock.getByRole('button', { name: 'Unlock', exact: true }).click()
  await expect(pane.getByTestId('enc-state')).toHaveText('Unlocked')
})
