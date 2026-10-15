import { expect, test } from './fixtures'

// TK-15: `/followup` marks the task done and creates the next one, linked with `follows::`; the chain is shown.
test('/followup finishes the task, creates a linked follow-up and the chain can be opened', async ({
  backend,
  page,
}) => {
  backend.write(
    'pages/proj.md',
    '- TODO write report #work [[Acme]]\n  priority:: A\n  scheduled:: 2026-10-05\n',
  )
  await backend.open('/page/proj')
  const blocks = page.getByRole('tree', { name: 'Blocks' })
  await blocks.getByText('write report').click()
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.press('Control+End')
  await page.keyboard.type(' /followup')
  const tip = page.locator('.cm-tooltip-autocomplete')
  await expect(tip).toContainText('Done + follow-up', { timeout: 5000 })
  await page.waitForTimeout(150) // CodeMirror ignores an accept within 75 ms of the popup opening
  await page.keyboard.press('Enter')

  // the new task is open and ready to type in
  await expect(page.getByTestId('block-editor')).toContainText('TODO #work [[Acme]]')
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.keyboard.type('send the invoice')
  await page.keyboard.press('Escape')

  await expect
    .poll(() => backend.read('pages/proj.md'))
    .toMatch(/^- DONE write report #work \[\[Acme\]\] \^([a-z0-9]{6})$/m)
  const text = backend.read('pages/proj.md')
  const id = /\^([a-z0-9]{6})$/m.exec(text)![1]
  // the follow-up: tags and links and priority carried, no dates, linked back
  expect(text).toMatch(
    new RegExp(
      `- TODO #work \\[\\[Acme\\]\\] send the invoice\\n  priority:: A\\n  follows:: \\(\\(${id}\\)\\)\\n`,
    ),
  )
  expect(text.match(/scheduled::/g)).toHaveLength(1)

  // the chain is one click away, from either task
  await page.getByTestId('task-chain').first().getByRole('button').click()
  const list = page.getByRole('group', { name: 'Chain of tasks' }).first()
  await expect(list.getByTestId('chain-item')).toHaveCount(2)
  await expect(list.getByTestId('chain-position')).toContainText('of 2')
  await expect(list.getByTestId('chain-item').nth(0)).toContainText('write report')
  await expect(list.getByTestId('chain-item').nth(1)).toContainText('send the invoice')

  // and the query clause finds both
  backend.write('pages/q.md', `- \`\`\`query\n  chain: ((${id}))\n  \`\`\`\n`)
  await backend.open('/page/q')
  await expect(page.getByTestId('query-count')).toHaveText('2 results')
})
