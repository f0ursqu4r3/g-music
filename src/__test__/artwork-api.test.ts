import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}))

const id = 'M7lc1UVf-VE'
let api: typeof import('../api').artworkApi

beforeEach(async () => {
  vi.resetModules()
  vi.useFakeTimers()
  vi.mocked(invoke).mockReset().mockResolvedValue('/artwork/image.jpg')
  vi.mocked(convertFileSrc).mockClear()
  api = (await import('../api')).artworkApi
})
afterEach(() => vi.useRealTimers())

describe('artworkApi cache', () => {
  it('shares concurrent requests and reuses warm paths', async () => {
    let finish!: (path: string) => void
    vi.mocked(invoke).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve
      }),
    )
    const first = api.resolveYouTube(id)
    const second = api.resolveYouTube(id)
    expect(first).toBe(second)
    finish('/artwork/image.jpg')
    await expect(first).resolves.toBe('asset:///artwork/image.jpg')
    await expect(api.resolveYouTube(id)).resolves.toBe('asset:///artwork/image.jpg')
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(invoke).toHaveBeenCalledWith('resolve_youtube_artwork', { videoId: id })
  })

  it('expires successful paths after 30 seconds', async () => {
    await api.resolveYouTube(id)
    vi.advanceTimersByTime(30_000)
    await api.resolveYouTube(id)
    expect(invoke).toHaveBeenCalledTimes(2)
  })

  it.each([null, new Error('offline')])('retries failures after 2 seconds: %s', async (failure) => {
    if (failure instanceof Error) vi.mocked(invoke).mockRejectedValue(failure)
    else vi.mocked(invoke).mockResolvedValue(null)
    await expect(api.resolveYouTube(id)).resolves.toBeNull()
    await api.resolveYouTube(id)
    expect(invoke).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(2_000)
    vi.mocked(invoke).mockResolvedValue('/artwork/recovered.jpg')
    await expect(api.resolveYouTube(id)).resolves.toContain('recovered.jpg')
    expect(invoke).toHaveBeenCalledTimes(2)
  })

  it('evicts the least recently used result at 256 entries', async () => {
    const ids = Array.from({ length: 257 }, (_, i) => String(i).padStart(11, '0'))
    for (const key of ids.slice(0, 256)) await api.resolveYouTube(key)
    await api.resolveYouTube(ids[0]!)
    await api.resolveYouTube(ids[256]!)
    await api.resolveYouTube(ids[0]!)
    expect(invoke).toHaveBeenCalledTimes(257)
    await api.resolveYouTube(ids[1]!)
    expect(invoke).toHaveBeenCalledTimes(258)
  })

  it('queues a large view without dropping artwork or duplicate promises', async () => {
    let finish!: (path: string) => void
    vi.mocked(invoke).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve
      }),
    )
    const requests = Array.from({ length: 160 }, (_, i) =>
      api.resolveYouTube(String(i).padStart(11, '0')),
    )
    expect(api.resolveYouTube('00000000159')).toBe(requests[159])
    expect(invoke).toHaveBeenCalledTimes(8)
    finish('/artwork/image.jpg')
    const results = await Promise.all(requests)
    expect(results).toHaveLength(160)
    expect(results.every((value) => value === 'asset:///artwork/image.jpg')).toBe(true)
    expect(invoke).toHaveBeenCalledTimes(160)
  })

  it('invalidates a broken path without letting an older request refill the cache', async () => {
    let finish!: (path: string) => void
    vi.mocked(invoke).mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve
      }),
    )
    const old = api.resolveYouTube(id)
    api.invalidateYouTube(id)
    vi.mocked(invoke).mockResolvedValue('/artwork/new.jpg')
    await api.resolveYouTube(id)
    finish('/artwork/old.jpg')
    await old
    await expect(api.resolveYouTube(id)).resolves.toContain('new.jpg')
    api.invalidateYouTube(id)
    await api.resolveYouTube(id)
    expect(invoke).toHaveBeenCalledTimes(3)
  })

  it.each(['', '../bad', ` ${id}`, `${id}\n`, `${id}x`, 'é'.repeat(11)])(
    'rejects non-exact IDs: %j',
    async (key) => {
      await expect(api.resolveYouTube(key)).resolves.toBeNull()
      expect(invoke).not.toHaveBeenCalled()
    },
  )
})
