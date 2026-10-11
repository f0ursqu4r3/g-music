import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string, protocol: string) => `${protocol}://localhost/${path}`,
}))
const { VisualizerVideo, isVideoId } = await import('../video')

function media(video: InstanceType<typeof VisualizerVideo>, state: Record<string, unknown>) {
  for (const [name, value] of Object.entries(state))
    Object.defineProperty(video.element, name, { configurable: true, writable: true, value })
}

describe('VisualizerVideo', () => {
  let play: ReturnType<typeof vi.fn>
  let pause: ReturnType<typeof vi.fn>
  beforeEach(() => {
    play = vi.fn(async () => {})
    pause = vi.fn()
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(play)
    vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(pause)
    vi.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {})
  })
  afterEach(() => vi.restoreAllMocks())

  it('accepts only YouTube video IDs', () => {
    expect(isVideoId('dQw4w9WgXcQ')).toBe(true)
    expect(isVideoId('local-track-1')).toBe(false)
    expect(isVideoId('../dQw4w9Wg')).toBe(false)
  })

  it('reports loading, missing and stopped video', () => {
    const states: string[] = []
    const video = new VisualizerVideo((state) => states.push(state))
    video.load('dQw4w9WgXcQ')
    expect(video.element.src).toBe('gmvideo://localhost/dQw4w9WgXcQ')
    video.load('not a video')
    video.load(null)
    expect(states).toEqual(['loading', 'unavailable', 'off'])
  })

  it('seeks on large drift and nudges the rate on small drift', () => {
    const video = new VisualizerVideo(() => {})
    video.load('dQw4w9WgXcQ')
    media(video, { readyState: 4, duration: 200, currentTime: 10, seeking: false, paused: true })
    video.sync({ positionMs: 20_000, playing: true }, 0, 0)
    expect(video.element.currentTime).toBe(20)
    expect(play).toHaveBeenCalled()

    media(video, { currentTime: 20.1, paused: false })
    video.sync({ positionMs: 20_000, playing: true }, 0, 500)
    expect(video.element.currentTime).toBe(20.1)
    expect(video.element.playbackRate).toBeLessThan(1)
  })

  it('applies the sync offset and mirrors pause', () => {
    const video = new VisualizerVideo(() => {})
    video.load('dQw4w9WgXcQ')
    media(video, { readyState: 4, duration: 200, currentTime: 0, seeking: false, paused: false })
    video.sync({ positionMs: 10_000, playing: false }, 1000, 0)
    expect(video.element.currentTime).toBe(9)
    expect(pause).toHaveBeenCalled()
  })

  it('pauses past the end of the video and without a playhead', () => {
    const video = new VisualizerVideo(() => {})
    video.load('dQw4w9WgXcQ')
    media(video, { readyState: 4, duration: 30, currentTime: 0, seeking: false, paused: false })
    pause.mockClear()
    video.sync({ positionMs: 40_000, playing: true }, 0, 0)
    video.sync(null, 0, 1000)
    expect(pause).toHaveBeenCalledTimes(2)
    expect(play).not.toHaveBeenCalled()
  })
})
