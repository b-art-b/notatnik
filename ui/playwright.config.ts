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
  use: { channel: 'chrome', baseURL: 'http://127.0.0.1:5173', trace: 'retain-on-failure' },
  webServer: {
    command: 'pnpm dev',
    url: 'http://127.0.0.1:5173',
    reuseExistingServer: true,
    timeout: 60_000,
  },
})
