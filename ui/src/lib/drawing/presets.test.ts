import { beforeEach, describe, expect, it } from 'vitest'
import {
  DEFAULT_PRESETS,
  PRESETS_KEY,
  describePreset,
  loadPresets,
  presetOf,
  samePen,
  savePresets,
  validPresets,
} from './presets'

describe('presetOf and samePen (DR-15)', () => {
  it('turns a js-draw colour and width into a preset', () => {
    expect(presetOf({ r: 1, g: 0.5, b: 0, a: 0.4 }, 12.001)).toEqual({
      color: '#ff8000',
      width: 12,
      alpha: 0.4,
    })
  })
  it('compares pens with a tolerance for floats, and by colour case-insensitively', () => {
    const p = { color: '#D11F1F', width: 3, alpha: 1 }
    expect(samePen(p, { color: '#d11f1f', width: 3.01, alpha: 0.99 })).toBe(true)
    expect(samePen(p, { color: '#d11f1f', width: 4, alpha: 1 })).toBe(false)
    expect(samePen(p, { color: '#d11f1e', width: 3, alpha: 1 })).toBe(false)
    expect(samePen(p, { color: '#d11f1f', width: 3, alpha: 0.4 })).toBe(false)
  })
})

describe('validPresets', () => {
  it('accepts the defaults and refuses anything of another shape', () => {
    expect(validPresets(DEFAULT_PRESETS)).toEqual(DEFAULT_PRESETS)
    expect(validPresets(null)).toBeNull()
    expect(validPresets([])).toBeNull()
    expect(validPresets(DEFAULT_PRESETS.slice(1))).toBeNull()
    expect(validPresets(DEFAULT_PRESETS.map((p) => ({ ...p, color: 'red' })))).toBeNull()
    expect(validPresets(DEFAULT_PRESETS.map((p) => ({ ...p, width: -1 })))).toBeNull()
    expect(validPresets(DEFAULT_PRESETS.map((p) => ({ ...p, alpha: 0 })))).toBeNull()
  })
})

describe('loadPresets and savePresets', () => {
  beforeEach(() => localStorage.clear())
  it('starts with the defaults', () => {
    expect(loadPresets()).toEqual(DEFAULT_PRESETS)
  })
  it('remembers a changed slot, and falls back to the defaults for text that is not a list of presets', () => {
    const p = loadPresets()
    p[2] = { color: '#00ff00', width: 5, alpha: 1 }
    savePresets(p)
    expect(loadPresets()[2]).toEqual({ color: '#00ff00', width: 5, alpha: 1 })
    localStorage.setItem(PRESETS_KEY, '{nope')
    expect(loadPresets()).toEqual(DEFAULT_PRESETS)
  })
})

describe('describePreset', () => {
  it('calls a pen with a low opacity a highlighter', () => {
    expect(describePreset({ color: '#ffd400', width: 18, alpha: 0.4 })).toMatch(/^Highlighter/)
    expect(describePreset({ color: '#000000', width: 2, alpha: 1 })).toMatch(/^Pen/)
  })
})
