import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, type Plugin } from 'vitest/config'
import fs from 'node:fs'
import path from 'node:path'

// pdf.js reads CJK character maps and the 14 standard fonts from files (PA-1). They are served under `/pdfjs/` in
// development and copied into the build; only a PDF that needs them fetches them.
function pdfjsFiles(): Plugin {
  const root = path.resolve('node_modules/pdfjs-dist')
  const dirs = ['cmaps', 'standard_fonts']
  return {
    name: 'pdfjs-files',
    configureServer(server) {
      server.middlewares.use('/pdfjs', (req, res, next) => {
        const file = path.join(root, decodeURIComponent((req.url ?? '').split('?')[0]))
        if (!file.startsWith(root) || !fs.existsSync(file) || !fs.statSync(file).isFile()) return next()
        res.end(fs.readFileSync(file))
      })
    },
    generateBundle() {
      for (const d of dirs)
        for (const f of fs.readdirSync(path.join(root, d)))
          this.emitFile({
            type: 'asset',
            fileName: `pdfjs/${d}/${f}`,
            source: fs.readFileSync(path.join(root, d, f)),
          })
    },
  }
}

export default defineConfig({
  plugins: [svelte(), pdfjsFiles()],
  // Tauri serves the built files from disk, so asset URLs must be relative.
  base: './',
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: '127.0.0.1' },
  preview: { port: 4173, strictPort: true, host: '127.0.0.1' },
  build: { target: 'es2023', sourcemap: true },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
  },
})
