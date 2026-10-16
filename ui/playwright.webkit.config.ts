import { defineConfig } from '@playwright/test'
import base from './playwright.config'

// The same tests in WebKit, the engine of the desktop app (see scripts/check-webkit.sh).
export default defineConfig({
  ...base,
  retries: 1,
  workers: 2,
  use: { baseURL: 'http://127.0.0.1:4173', browserName: 'webkit', trace: 'retain-on-failure' },
})
