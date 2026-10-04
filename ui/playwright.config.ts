import { defineConfig } from '@playwright/test'

// UI tests run in system Chrome against the real Rust backend (notatnik-dev-server), one server
// and one temp copy of a fixture graph per test. See tests/fixtures.ts.
export default defineConfig({
  testDir: 'tests',
  timeout: 30_000,
  fullyParallel: true,
  // Known flake (~1 in 10): the first Ctrl+Z after Esc Esc in editing.spec.ts occasionally doesn't reach the app. See docs/TODO.md.
  retries: 2,
  workers: 4,
  reporter: [['list']],
  globalSetup: './tests/global-setup.ts',
  use: { channel: 'chrome', baseURL: 'http://127.0.0.1:4173', trace: 'retain-on-failure' },
  webServer: {
    // the production build is faster and steadier under parallel workers than the dev server
    command: 'pnpm build && pnpm preview',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: false,
    timeout: 60_000,
  },
})
