import { describe, expect, it } from 'vitest'

import { mockWindowViews, resolveMockWindowView, type MockWindowView } from '../window-view'

describe('mock window routes', () => {
  it('resolves each supported desktop surface', () => {
    const expected: MockWindowView[] = ['library', 'artwork', 'queue', 'mini', 'settings', 'import']

    expect(mockWindowViews).toEqual(expected)
    expect(resolveMockWindowView('?view=artwork')).toBe('artwork')
    expect(resolveMockWindowView('?view=mini')).toBe('mini')
    expect(resolveMockWindowView('?view=settings')).toBe('settings')
    expect(resolveMockWindowView('?view=import')).toBe('import')
    expect(resolveMockWindowView('?view=albums')).toBe('library')
    expect(resolveMockWindowView('?view=artists')).toBe('library')
  })

  it('uses the library surface for an unsupported route', () => {
    expect(resolveMockWindowView('?view=unknown')).toBe('library')
    expect(resolveMockWindowView('')).toBe('library')
  })
})
