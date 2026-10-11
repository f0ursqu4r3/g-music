import type { VisualizerFrame, VisualizerSnapshot } from './api'

export interface VisualizerSample {
  bands: Float32Array
  timeSeconds: number
}
const neutral = () => new Float32Array(16)
/** Media timestamps are authoritative. IPC arrival time only locates a clock observation. */
export class VisualizerTimeline {
  private generation = -1
  private frames: VisualizerFrame[] = []
  private status: VisualizerSnapshot['status'] = 'idle'
  private position: number | null = null
  private anchor = 0
  private received = -Infinity
  private advancing = true
  private last: VisualizerSample = { bands: neutral(), timeSeconds: 0 }
  private missingSince: number | null = null
  private fadeSource = neutral()
  get frameCount(): number {
    return this.frames.length
  }
  reset(): void {
    this.frames = []
    this.position = null
    this.status = 'idle'
    this.received = -Infinity
    this.last = { bands: neutral(), timeSeconds: 0 }
    this.missingSince = null
  }
  observe(snapshot: VisualizerSnapshot, requestStart: number, requestEnd: number): void {
    if (!Number.isFinite(snapshot.generation) || snapshot.generation < this.generation) return
    if (snapshot.generation !== this.generation) {
      this.reset()
      this.generation = snapshot.generation
    }
    const midpoint = (requestStart + requestEnd) / 2
    const position = snapshot.audioPositionMs
    this.advancing =
      this.status !== 'playing' ||
      this.position === null ||
      (position !== null && position > this.position + 0.1)
    this.position = position !== null && Number.isFinite(position) ? position : null
    this.anchor = midpoint
    this.received = requestEnd
    this.status = snapshot.status
    const merged = new Map(this.frames.map((frame) => [frame.positionMs, frame]))
    for (const frame of snapshot.frames.slice(-256)) {
      if (
        Number.isFinite(frame.positionMs) &&
        frame.positionMs >= 0 &&
        frame.bands.length === 16 &&
        frame.bands.every((b) => Number.isFinite(b) && b >= 0 && b <= 1)
      )
        merged.set(frame.positionMs, { positionMs: frame.positionMs, bands: [...frame.bands] })
    }
    this.frames = [...merged.values()].sort((a, b) => a.positionMs - b.positionMs).slice(-256)
    if (this.position !== null)
      this.frames = this.frames.filter((frame) => frame.positionMs >= this.position! - 1500)
  }
  /** Estimated media position for a video that follows the audio. Null when unknown. */
  playhead(now: number): { positionMs: number; playing: boolean } | null {
    if (this.position === null || (this.status !== 'playing' && this.status !== 'paused'))
      return null
    // A stalled poll must not let the video run ahead of audio that stopped.
    const playing = this.status === 'playing' && now - this.received <= 500
    const elapsed = playing && this.advancing ? Math.min(150, Math.max(0, now - this.anchor)) : 0
    return { positionMs: this.position + elapsed, playing }
  }
  sample(now: number, offsetMs: number): VisualizerSample {
    const fresh = now - this.received <= 250
    const elapsed =
      this.status === 'playing' && this.advancing
        ? Math.min(150, Math.max(0, now - this.anchor))
        : 0
    const position = this.position === null ? null : this.position + elapsed - offsetMs
    const usable = (this.status === 'playing' && fresh) || this.status === 'paused'
    if (usable && position !== null) {
      const frame = this.frames.find(
        (frame) => position >= frame.positionMs && position < frame.positionMs + 20,
      )
      if (frame) {
        this.missingSince = null
        this.last = {
          bands: new Float32Array(frame.bands),
          timeSeconds: (this.position! + elapsed) / 1000,
        }
        return this.last
      }
    }
    if (this.status === 'paused') return this.last
    if (this.missingSince === null) {
      this.missingSince = Math.min(now, this.received + 250)
      this.fadeSource = this.last.bands.slice()
    }
    const fade = Math.max(0, 1 - (now - this.missingSince) / 200)
    this.last = {
      bands: this.fadeSource.map((value) => value * fade),
      timeSeconds: this.last.timeSeconds,
    }
    return this.last
  }
}
