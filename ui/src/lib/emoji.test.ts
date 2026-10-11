import { describe, expect, it } from 'vitest'
import { EMOJIS, searchEmoji } from './emoji'

describe('emoji set', () => {
  it('has plenty of entries, each with a character and a shortcode', () => {
    expect(EMOJIS.length).toBeGreaterThan(300)
    for (const e of EMOJIS) {
      expect(e.char.length).toBeGreaterThan(0)
      expect(e.name).toMatch(/^[a-z0-9_+-]+$/)
    }
  })
  it('finds by shortcode prefix first, then by words', () => {
    expect(searchEmoji('smile')[0].char).toBe('😄')
    expect(searchEmoji('thumbsup')[0].char).toBe('👍')
    expect(searchEmoji('+1')[0].char).toBe('👍')
    expect(searchEmoji('done').map((e) => e.char)).toContain('✅')
    expect(searchEmoji('deadline').map((e) => e.char)).toContain('📅')
  })
  it('accepts a leading colon and spaces for underscores', () => {
    expect(searchEmoji(':heart_eyes')[0].char).toBe('😍')
    expect(searchEmoji('heart eyes')[0].char).toBe('😍')
  })
  it('lists each emoji once and respects the limit', () => {
    const r = searchEmoji('fire', 5)
    expect(r.length).toBeLessThanOrEqual(5)
    expect(new Set(r.map((e) => e.char)).size).toBe(r.length)
    expect(searchEmoji('zzzzzz')).toEqual([])
  })
})
