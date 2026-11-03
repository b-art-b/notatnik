import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DraftSaver, draftKey, type Draft, type DraftStore } from './draft'

function memoryStore(): DraftStore & { data: Map<string, Draft>; fail: boolean } {
  const data = new Map<string, Draft>()
  const s = {
    data,
    fail: false,
    get: async (k: string) => data.get(k),
    put: async (k: string, d: Draft) => {
      if (s.fail) throw new Error('no storage')
      data.set(k, d)
    },
    delete: async (k: string) => void data.delete(k),
  }
  return s
}

describe('draftKey (DR-13)', () => {
  it('is the drawing for an existing one, however its link climbs, and the page for a new one', () => {
    expect(draftKey({ path: '../assets/drawings/draw-0123abcd.svg', page: null })).toBe(
      'file:assets/drawings/draw-0123abcd.svg',
    )
    expect(draftKey({ path: null, page: 'inbox' })).toBe('new:inbox')
  })
})

describe('DraftSaver', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('writes once, a few seconds after the first change, however many changes come', async () => {
    const store = memoryStore()
    const put = vi.spyOn(store, 'put')
    let n = 0
    const saver = new DraftSaver(
      store,
      'k',
      () => `<svg>${++n}</svg>`,
      3000,
      () => 42,
    )
    saver.touch()
    saver.touch()
    await vi.advanceTimersByTimeAsync(2999)
    expect(put).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(2)
    expect(put).toHaveBeenCalledTimes(1)
    expect(store.data.get('k')).toEqual({ svg: '<svg>1</svg>', savedAt: 42 })
    // the next change starts a new wait
    saver.touch()
    await vi.advanceTimersByTimeAsync(3001)
    expect(put).toHaveBeenCalledTimes(2)
  })

  it('removes the draft when there is nothing to keep any more (drawn back to how it was)', async () => {
    const store = memoryStore()
    store.data.set('k', { svg: 'old', savedAt: 1 })
    const saver = new DraftSaver(store, 'k', () => null)
    saver.touch()
    await vi.advanceTimersByTimeAsync(3001)
    expect(store.data.has('k')).toBe(false)
  })

  it('flush writes at once, and clear removes the draft and stops all writing', async () => {
    const store = memoryStore()
    const saver = new DraftSaver(store, 'k', () => '<svg/>')
    await saver.flush()
    expect(store.data.has('k')).toBe(true)
    saver.touch()
    await saver.clear()
    expect(store.data.has('k')).toBe(false)
    await vi.advanceTimersByTimeAsync(10_000)
    await saver.flush()
    expect(store.data.has('k')).toBe(false)
  })

  it('does not reject when the storage fails', async () => {
    const store = memoryStore()
    store.fail = true
    const saver = new DraftSaver(store, 'k', () => '<svg/>')
    await expect(saver.flush()).resolves.toBeUndefined()
  })
})
