import type { Page } from '@playwright/test'

/** Opens a part of a long Settings section (the row of links under its title), e.g. `openPart(page, 'Sync', 'Encryption')`. */
export async function openPart(page: Page, section: 'Sync' | 'Calendar', part: string) {
  await page
    .getByRole('navigation', { name: `${section} sections` })
    .getByRole('button', { name: part, exact: true })
    .click()
}
