// Performance checks (NF-4, P2.29, P3.7). Slow; run with `PERF=1 pnpm test:e2e tests/perf.spec.ts` (scripts/check.sh --full).
// They run in headless Chrome. The real target is WebKitGTK: repeat them there by hand (spike S1).
import { expect, test } from './fixtures'

test.skip(!process.env.PERF, 'set PERF=1 to run the performance checks')

const big = (n: number) =>
  Array.from(
    { length: n },
    (_, i) =>
      `- Block ${i} with **bold**, a [[link ${i % 50}]] and #tag${i % 7}${i % 5 === 0 ? '\n  status:: active' : ''}${i % 40 === 0 ? '\n  - child of ' + i : ''}`,
  ).join('\n') + '\n'

test('a 2,000-block page opens quickly and typing stays under one frame', async ({ backend, page }) => {
  backend.write('pages/big.md', big(2000))
  await backend.open('/journals')
  const open = await page.evaluate(
    () =>
      new Promise<number>((resolve) => {
        const t0 = performance.now()
        const done = () => {
          if (document.body.textContent?.includes('Block 5 with')) {
            obs.disconnect()
            requestAnimationFrame(() => resolve(performance.now() - t0))
          }
        }
        const obs = new MutationObserver(done)
        obs.observe(document.body, { childList: true, subtree: true })
        window.location.hash = '#/page/big'
      }),
  )
  console.log(`open 2000 blocks: ${open.toFixed(0)} ms`)
  expect(open).toBeLessThan(300)
  // the rest streams in within the next frames
  await page.waitForFunction(() => document.querySelectorAll('[data-block-id]').length >= 2050, undefined, {
    timeout: 5000,
  })

  await page.getByText('Block 1000 with').click({ position: { x: 5, y: 5 } })
  await expect(page.getByTestId('block-editor')).toBeFocused()
  await page.evaluate(() => {
    const w = window as unknown as { __lat: number[] }
    w.__lat = []
    let t0 = 0
    document.addEventListener('keydown', () => (t0 = performance.now()), true)
    document.addEventListener(
      'input',
      () => {
        requestAnimationFrame(() => requestAnimationFrame(() => w.__lat.push(performance.now() - t0)))
      },
      true,
    )
  })
  for (let i = 0; i < 30; i++) await page.keyboard.type('x', { delay: 40 })
  await page.waitForTimeout(300)
  const lat = (await page.evaluate(() => (window as unknown as { __lat: number[] }).__lat)).sort(
    (a, b) => a - b,
  )
  const median = lat[Math.floor(lat.length / 2)]
  const p95 = lat[Math.floor(lat.length * 0.95)]
  console.log(
    `keystroke to paint on 2000 blocks: median ${median.toFixed(1)} ms, p95 ${p95.toFixed(1)} ms (two frames of rAF included)`,
  )
  expect(median).toBeLessThan(40)
})

test('live query results update within 200 ms of an edit', async ({ backend, page }) => {
  backend.write('pages/dash.md', '- ```query\n  tag: urgent\n  task: any\n  ```\n')
  backend.write('pages/many.md', Array.from({ length: 400 }, (_, i) => `- TODO task ${i}`).join('\n') + '\n')
  await backend.open('/page/dash')
  await expect(page.getByTestId('query-count')).toHaveText('0 results')
  const p = await backend.call<{ hash: string }>('get_page', { name: 'many' })
  const t0 = Date.now()
  await backend.call('apply_ops', {
    page: 'many',
    base_hash: p.hash,
    ops: [{ op: 'update_text', block: { path: [3] }, text: 'TODO task 3 #urgent' }],
  })
  await expect(page.getByTestId('query-count')).toHaveText('1 result', { timeout: 5000 })
  const ms = Date.now() - t0
  console.log(`edit to updated query result: ${ms} ms`)
  expect(ms).toBeLessThan(200 + 150) // 150 ms: the edit request itself and the test harness's own polling
})
