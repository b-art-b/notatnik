import { describe, expect, it } from 'vitest'
import { coveringPaths, pathIsEncrypted } from './encryption.svelte'

describe('pathIsEncrypted', () => {
  const rules = ['pages/private/', 'pages/Health.md']
  it('a folder covers everything below it and a file only itself', () => {
    for (const yes of ['pages/private/a.md', 'pages/private/deep/er/b.md', 'pages/Health.md'])
      expect(pathIsEncrypted(rules, yes), yes).toBe(true)
    for (const no of [
      'pages/private-x.md',
      'pages/privateer/a.md',
      'pages/private',
      'pages/health.md',
      'pages/Health/sub.md',
      'journals/2026-10-03.md',
    ])
      expect(pathIsEncrypted(rules, no), no).toBe(false)
  })
  it('nothing below .notatnik is encrypted, and a conflict copy follows its original', () => {
    expect(pathIsEncrypted(['.notatnik/'], '.notatnik/config.toml')).toBe(false)
    expect(pathIsEncrypted(rules, 'pages/Health (conflicted copy 2026-10-03 120000).md')).toBe(true)
    expect(pathIsEncrypted(rules, 'pages/private/x (conflicted copy 2026-10-03 120000).md')).toBe(true)
    expect(pathIsEncrypted(rules, 'pages/Other (conflicted copy 2026-10-03 120000).md')).toBe(false)
    expect(pathIsEncrypted([], 'pages/a.md')).toBe(false)
  })
  it('names the rules that cover a file', () => {
    expect(coveringPaths(rules, 'pages/private/a.md')).toEqual(['pages/private/'])
    expect(coveringPaths(rules, 'pages/Health.md')).toEqual(['pages/Health.md'])
    expect(coveringPaths(rules, 'pages/x.md')).toEqual([])
  })
})
