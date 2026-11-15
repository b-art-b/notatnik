import { expect, test } from './fixtures'

// ED-28: a rendered code block has a Copy button that puts the code alone on the clipboard.
test('Copy puts the code on the clipboard without the fences, and does not open the block for editing', async ({
  backend,
  context,
  page,
  browserName,
}) => {
  test.skip(browserName !== 'chromium', 'clipboard permissions are a Chromium feature of the test driver')
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  backend.write('pages/Code.md', '- intro\n- ```sh\n  lxc init a\n  # note\n  zfs create b\n  ```\n')
  await backend.open('/page/Code')
  const block = page.locator('.code-block')
  await expect(block).toBeVisible()
  await block.hover()
  await page.getByRole('button', { name: 'Copy code' }).click()
  await expect(page.getByRole('button', { name: 'Copy code' })).toHaveText('Copied')
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('lxc init a\n# note\nzfs create b')
  await expect(page.getByTestId('block-editor')).toHaveCount(0)
})
