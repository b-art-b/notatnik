import { expect, test } from './fixtures'

// Each task state has its own color, so the state is seen without reading the word.
test('the markers of the task states have different colors', async ({ backend, page }) => {
  backend.write('pages/states.md', '- TODO a\n- DOING b\n- WAITING c\n- DONE d\n- CANCELED e\n')
  await backend.open('/page/states')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  const dot = (state: string) =>
    blocks
      .getByRole('button', { name: new RegExp(`^${state}`, 'i') })
      .first()
      .evaluate(
        (el) =>
          getComputedStyle(el, '::before').backgroundColor + '|' + getComputedStyle(el, '::before').boxShadow,
      )
  const seen = new Set<string>()
  for (const s of ['TODO', 'DOING', 'WAITING', 'DONE', 'CANCELED']) seen.add(await dot(s))
  expect(seen.size).toBe(5)
})
