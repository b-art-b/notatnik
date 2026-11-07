import { describe, expect, it } from 'vitest'
import { toggleWrap, type Marker } from './formatText'

/** `‹x›` marks the selection in the input; the result is written the same way. */
function run(input: string, marker: Marker): string | null {
  const from = input.indexOf('‹')
  const to = input.indexOf('›') - 1
  const text = input.replace('‹', '').replace('›', '')
  const r = toggleWrap(text, from, to, marker)
  return r ? r.text.slice(0, r.from) + '‹' + r.text.slice(r.from, r.to) + '›' + r.text.slice(r.to) : null
}

describe('toggleWrap', () => {
  it('wraps a word, a phrase and several words', () => {
    expect(run('a ‹word› b', '**')).toBe('a **‹word›** b')
    expect(run('‹two words›', '*')).toBe('*‹two words›*')
    expect(run('x ‹a b c› y', '~~')).toBe('x ~~‹a b c›~~ y')
    expect(run('‹hi›', '==')).toBe('==‹hi›==')
    expect(run('‹x›', '`')).toBe('`‹x›`')
  })
  it('takes the markers off, outside or inside the selection', () => {
    expect(run('a **‹word›** b', '**')).toBe('a ‹word› b')
    expect(run('a ‹**word**› b', '**')).toBe('a ‹word› b')
    expect(run('~~‹x›~~', '~~')).toBe('‹x›')
    expect(run('‹==x==›', '==')).toBe('‹x›')
    expect(run('`‹x›`', '`')).toBe('‹x›')
  })
  it('keeps spaces at the ends of the selection outside the markers', () => {
    expect(run('a‹ word ›b', '**')).toBe('a **‹word›** b')
    expect(run('‹ word›', '*')).toBe(' *‹word›*')
  })
  it('does nothing for an empty or blank selection', () => {
    expect(toggleWrap('abc', 1, 1, '**')).toBeNull()
    expect(toggleWrap('a   b', 1, 4, '**')).toBeNull()
  })
  it('keeps bold and italic apart and combines them', () => {
    expect(run('**‹a›**', '*')).toBe('***‹a›***')
    expect(run('*‹a›*', '**')).toBe('***‹a›***')
    expect(run('***‹a›***', '*')).toBe('**‹a›**')
    expect(run('***‹a›***', '**')).toBe('*‹a›*')
    expect(run('*‹a›*', '*')).toBe('‹a›')
    expect(run('‹***a***›', '*')).toBe('‹**a**›')
  })
  it('wraps a link or a reference whole', () => {
    expect(run('see ‹[[Page]]› now', '**')).toBe('see **‹[[Page]]›** now')
    expect(run('‹((k3x9qa))›', '*')).toBe('*‹((k3x9qa))›*')
  })
  it('works line by line over a selection of several lines', () => {
    expect(run('‹one\ntwo›', '**')).toBe('**‹one**\n**two›**')
    expect(run('‹**one**\n**two**›', '**')).toBe('‹one\ntwo›')
    expect(run('‹one\n\ntwo›', '*')).toBe('*‹one*\n\n*two›*')
  })
  it('marks the lines that have no marker when only some have it', () => {
    expect(run('‹**one**\ntwo›', '**')).toBe('‹**one**\n**two›**')
  })
  it('the new selection is the text without the markers', () => {
    const r = toggleWrap('a word b', 2, 6, '**')!
    expect(r.text.slice(r.from, r.to)).toBe('word')
  })
})
