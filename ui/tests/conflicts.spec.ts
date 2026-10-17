import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from './fixtures'

// ST-6, P5.2: a conflict copy is merged with its page from the page as it was before the two diverged.
const COPY = 'pages/trip (conflicted copy 2026-10-17 123000).md'

/** A page, its conflict copy and the ancestor the sync would have kept for the copy. */
function conflict(
  backend: { dir: string; write(rel: string, text: string): void },
  page: string,
  copy: string,
  base: string,
) {
  backend.write('pages/trip.md', page)
  backend.write(COPY, copy)
  const f = path.join(path.dirname(backend.dir), 'graph.sync-bases', 'ancestor', COPY)
  fs.mkdirSync(path.dirname(f), { recursive: true })
  fs.writeFileSync(f, base)
}

test('changes that do not overlap are merged with one click and the copy goes to the trash', async ({
  backend,
  page,
}) => {
  await backend.open('/page/inbox')
  conflict(backend, '- pack\n- hotel server\n', '- pack bags\n- hotel\n', '- pack\n- hotel\n')
  await page.getByTestId('conflict-banner').getByRole('button', { name: COPY }).click()
  const r = page.getByTestId('conflict-resolver')
  await expect(r.getByTestId('conflict-clean')).toBeVisible()
  await expect(r.getByTestId('conflict-diff')).toContainText('+ - pack bags')
  await r.getByTestId('conflict-merge').click()
  await expect.poll(() => backend.read('pages/trip.md')).toBe('- pack bags\n- hotel server\n')
  await expect(page.getByTestId('conflict-banner')).toBeHidden()
  // the page is written first, then the copy moves
  await expect.poll(() => backend.exists(COPY)).toBe(false)
  expect(backend.exists(`.notatnik/trash/${COPY}`)).toBe(true)
})

test('a block both sides changed is decided by choosing, and the preview follows', async ({
  backend,
  page,
}) => {
  await backend.open('/page/inbox')
  conflict(backend, '- pack: mine\n- hotel\n', '- pack: theirs\n- hotel\n', '- pack\n- hotel\n')
  await page.getByTestId('conflict-banner').getByRole('button', { name: COPY }).click()
  const r = page.getByTestId('conflict-resolver')
  await expect(r.getByTestId('conflict-block')).toHaveCount(1)
  await r.getByRole('radio', { name: 'Both' }).check()
  await expect(r.getByTestId('conflict-diff')).toContainText('+ - pack: theirs')
  await r.getByRole('radio', { name: 'Edit…' }).check()
  await r.getByLabel('Edited block').fill('- pack: agreed\n')
  await r.getByLabel('Edited block').blur()
  await expect(r.getByTestId('conflict-diff')).toContainText('+ - pack: agreed')
  await r.getByTestId('conflict-merge').click()
  await expect.poll(() => backend.read('pages/trip.md')).toBe('- pack: agreed\n- hotel\n')
})

test('without a common ancestor the page is kept or the copy is taken', async ({ backend, page }) => {
  await backend.open('/page/inbox')
  backend.write('pages/trip.md', '- mine\n')
  backend.write(COPY, '- theirs\n')
  await page.getByTestId('conflict-banner').getByRole('button', { name: COPY }).click()
  const r = page.getByTestId('conflict-resolver')
  await expect(r).toContainText('cannot be merged')
  await expect(r.getByTestId('conflict-merge')).toHaveCount(0)
  await r.getByTestId('conflict-use-copy').click()
  await expect.poll(() => backend.read('pages/trip.md')).toBe('- theirs\n')
  await expect.poll(() => backend.exists(COPY)).toBe(false)
})
