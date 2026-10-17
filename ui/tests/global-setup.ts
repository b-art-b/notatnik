import { fileURLToPath } from 'node:url'
import { execSync } from 'node:child_process'
import path from 'node:path'

export default function setup() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
  execSync('cargo build -q -p notatnik-dev-server', {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` },
  })
  // the in-memory WebDAV server of the sync tests
  execSync('cargo build -q -p notatnik-sync --features fakedav --bin notatnik-fakedav', {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` },
  })
}
