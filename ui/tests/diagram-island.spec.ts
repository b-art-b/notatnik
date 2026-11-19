import { spawn, type ChildProcess } from 'node:child_process'
import { expect, test } from '@playwright/test'

// X2 (DG-2, DG-5, DG-6): Excalidraw is an island that loads on demand, works with the network blocked, and a scene
// stored in an SVG comes back as separate shapes. The island is mounted on its own page of the Vite dev server (the
// overlay around it is X3), so this file starts that server itself.

const PORT = 5199
const BASE = `http://127.0.0.1:${PORT}`
let server: ChildProcess

test.beforeAll(async () => {
  server = spawn('pnpm', ['exec', 'vite', '--port', String(PORT), '--strictPort'], {
    cwd: process.cwd(),
    stdio: 'ignore',
  })
  const deadline = Date.now() + 40_000
  for (;;) {
    try {
      if ((await fetch(`${BASE}/tests/harness/diagram.html`)).ok) break
    } catch {
      /* not up yet */
    }
    if (Date.now() > deadline) throw new Error('the Vite dev server did not start')
    await new Promise((r) => setTimeout(r, 300))
  }
})
test.afterAll(() => {
  server?.kill()
})

test.describe.configure({ mode: 'serial' })
test.setTimeout(120_000)

type Handle = {
  getSvg(): Promise<string>
  isEmpty(): boolean
  changed(): boolean
  markSaved(): void
  unmount(): void
}
declare global {
  interface Window {
    mountDiagram(opts: { svg?: string; theme: 'light' | 'dark' }): Promise<Handle>
    handle: Handle
    harnessReady: boolean
  }
}

test('works with the network blocked: draws, exports an SVG with the scene, and reads it back as shapes', async ({
  page,
}) => {
  // everything that does not come from the dev server is refused, and counted
  const outside: string[] = []
  await page.route('**/*', (route) => {
    const url = route.request().url()
    if (url.startsWith(BASE) || url.startsWith('data:') || url.startsWith('blob:')) return route.continue()
    outside.push(url)
    return route.abort()
  })
  await page.goto(`${BASE}/tests/harness/diagram.html`)
  await page.waitForFunction(() => window.harnessReady)

  await page.evaluate(async () => {
    window.handle = await window.mountDiagram({ theme: 'light' })
  })
  const canvas = page.locator('.excalidraw canvas.interactive')
  await expect(canvas).toBeVisible()
  // the rectangle tool, then a drag on the canvas
  await page.getByTestId('toolbar-rectangle').click({ force: true })
  const box = (await canvas.boundingBox())!
  await page.mouse.move(box.x + 400, box.y + 300)
  await page.mouse.down()
  await page.waitForTimeout(100)
  await page.mouse.move(box.x + 500, box.y + 340, { steps: 10 })
  await page.mouse.move(box.x + 560, box.y + 390, { steps: 10 })
  await page.mouse.up()
  await page.waitForTimeout(200)
  expect(await page.evaluate(() => window.handle.isEmpty())).toBe(false)
  expect(await page.evaluate(() => window.handle.changed())).toBe(true)

  const svg = await page.evaluate(() => window.handle.getSvg())
  expect(svg.startsWith('<svg')).toBe(true)
  expect(svg).toContain('payload-type:application/vnd.excalidraw+json')
  // the picture is on white whatever the theme
  expect(svg).toMatch(/fill="#ffffff"/i)

  // the editor does not offer the hosted features
  await expect(page.locator('.excalidraw .library-menu-browse-button')).toHaveCount(0)

  // reopened from the SVG: the rectangle is still a shape of its own (selectable, not a flat picture)
  await page.evaluate(() => window.handle.unmount())
  await page.evaluate(async (s) => {
    document.getElementById('host')!.innerHTML = ''
    window.handle = await window.mountDiagram({ svg: s, theme: 'light' })
  }, svg)
  await expect(canvas).toBeVisible()
  expect(await page.evaluate(() => window.handle.isEmpty())).toBe(false)
  const again = await page.evaluate(() => window.handle.getSvg())
  const count = (t: string) => (t.match(/<rect/g) ?? []).length
  expect(count(again)).toBeGreaterThanOrEqual(1)
  // nothing but the dev server was asked for anything (fonts come from /excalidraw/fonts/)
  expect(outside).toEqual([])
})

test('nothing of Excalidraw is loaded until a diagram is opened', async ({ page }) => {
  const loaded: string[] = []
  page.on('request', (r) => loaded.push(r.url()))
  await page.goto(`${BASE}/tests/harness/diagram.html`)
  await page.waitForFunction(() => window.harnessReady)
  // the harness imports the island module, which imports React and Excalidraw only inside `mountDiagram`
  expect(loaded.some((u) => /@excalidraw|\/react(-dom)?[./@]/.test(u) && !u.includes('excalidraw.ts'))).toBe(
    false,
  )
})
