import { expect, test } from './fixtures'
import { openPart } from './parts'

// Every switch of Settings looks like a switch, wherever its section is drawn (the style is shared, not scoped to one
// component), and the one rule: what applies at once is a switch, what is saved with a button is a checkbox.
const looksLikeSwitch = (el: Element) => {
  const cs = getComputedStyle(el)
  return { appearance: cs.appearance, width: Math.round(el.getBoundingClientRect().width) }
}

test('switches look the same in every section of Settings', async ({ backend, page }) => {
  await backend.open('/settings')
  // the first section draws its switches itself
  const first = page.locator('input[role="switch"]').first()
  await expect(first).toBeVisible()
  const reference = await first.evaluate(looksLikeSwitch)
  expect(reference.appearance).toBe('none')

  // a pane that is a component of its own, with the key set up
  await backend.call('encryption_setup', { password: 'correct horse' })
  await backend.call('encryption_confirm', {})
  await page.getByRole('tab', { name: /Sync/ }).click()
  await openPart(page, 'Sync', 'Encryption')
  const switches = page.locator('input[role="switch"]')
  await expect(switches).toHaveCount(3) // keyring, hide names, calendar tasks
  for (const sw of await switches.all()) expect(await sw.evaluate(looksLikeSwitch)).toEqual(reference)
})
