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

// Excalidraw (DG-5) loads its fonts from `window.EXCALIDRAW_ASSET_PATH` (`/excalidraw/`) and otherwise from a CDN: the
// fonts are served from the package in development and copied into the build, so a diagram needs no network. The big
// CJK font (Xiaolai, 13 MB) is left out: text in Chinese, Japanese or Korean falls back to the system's fonts.
function excalidrawFiles(): Plugin {
  const root = path.resolve('node_modules/@excalidraw/excalidraw/dist/prod/fonts')
  const skip = new Set(['Xiaolai'])
  const files = (): string[] => {
    const out: string[] = []
    for (const d of fs.readdirSync(root)) {
      if (skip.has(d)) continue
      for (const f of fs.readdirSync(path.join(root, d))) out.push(`${d}/${f}`)
    }
    return out
  }
  return {
    name: 'excalidraw-files',
    configureServer(server) {
      server.middlewares.use('/excalidraw/fonts', (req, res, next) => {
        const file = path.join(root, decodeURIComponent((req.url ?? '').split('?')[0]))
        if (!file.startsWith(root) || !fs.existsSync(file) || !fs.statSync(file).isFile()) return next()
        res.end(fs.readFileSync(file))
      })
    },
    generateBundle() {
      for (const f of files())
        this.emitFile({
          type: 'asset',
          fileName: `excalidraw/fonts/${f}`,
          source: fs.readFileSync(path.join(root, f)),
        })
    },
  }
}

export default defineConfig({
  plugins: [svelte(), pdfjsFiles(), excalidrawFiles()],
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
