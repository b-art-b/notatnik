import { describe, expect, it } from 'vitest'
import { carriedTokens, followUpText } from './exec'

describe('/followup', () => {
  it('carries tags and page links, not dates, and each once', () => {
    expect(carriedTokens('TODO write report #work [[Acme]] on [[2026-10-14]] #work')).toEqual([
      '#work',
      '[[Acme]]',
    ])
    expect(carriedTokens('TODO call #[[big client]] about foo')).toEqual(['#[[big client]]'])
    expect(carriedTokens('TODO nothing here')).toEqual([])
    // an email or a heading is not a tag
    expect(carriedTokens('TODO mail a#b to x')).toEqual([])
  })

  it('builds an open task that links back, with the priority, the cursor ending the first line', () => {
    expect(followUpText('TODO write #work', 'A', 'abc123')).toEqual({
      text: 'TODO #work \npriority:: A\nfollows:: ((abc123))',
      cursor: 11,
    })
    expect(followUpText('TODO plain', null, 'abc123')).toEqual({
      text: 'TODO \nfollows:: ((abc123))',
      cursor: 5,
    })
  })
})
