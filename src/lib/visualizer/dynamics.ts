import type { VisualizerSample } from './timeline'

export interface VisualizerSignal {
  /** Auto-gained, smoothed band levels. */
  bands: Float32Array
  /** Peak caps that hold, then fall with gravity. */
  peaks: Float32Array
  /** Bass, low mids, high mids and treble. Each is 0 to 1. */
  levels: [number, number, number, number]
  /** Kick, snare and hi-hat pulses. Each is 1 on an onset, then decays toward 0. */
  hits: [number, number, number]
  /** The kick pulse. Kept for scenes and effects that need one beat. */
  beat: number
  /** Short-term loudness against the last several seconds. 0.5 is steady; above is a build or drop. */
  swell: number
  /** Scene clock. It runs faster when the music is louder and stops when media stops. */
  timeSeconds: number
}

const BANDS = 16
// Bands follow the analysis crossover: 0-3 are below 180 Hz, 12-15 are above 4.5 kHz.
const LEVEL_RANGES = [
  [0, 4],
  [4, 8],
  [8, 12],
  [12, 16],
] as const

/** Flux against an adaptive threshold. Each instrument has its own range and decay. */
class OnsetDetector {
  private history: number[] = []
  private since = Infinity
  pulse = 0
  constructor(
    private from: number,
    private to: number,
    private decay: number,
    private minimum: number,
    private gap: number,
  ) {}
  update(current: Float32Array, previous: Float32Array, dt: number): void {
    let flux = 0
    for (let i = this.from; i < this.to; i++) flux += Math.max(0, current[i]! - previous[i]!)
    flux /= this.to - this.from
    const count = Math.max(1, this.history.length)
    const mean = this.history.reduce((sum, value) => sum + value, 0) / count
    const deviation = Math.sqrt(
      this.history.reduce((sum, value) => sum + (value - mean) ** 2, 0) / count,
    )
    this.history.push(flux)
    if (this.history.length > 43) this.history.shift()
    this.since += dt
    this.pulse *= Math.exp(-dt * this.decay)
    if (flux > this.minimum && flux > mean + deviation * 1.6 && this.since > this.gap) {
      this.pulse = 1
      this.since = 0
    }
  }
}

/**
 * Raw RMS levels span a narrow range for most mastered music. This stage stretches
 * each band to its recent range, shapes attack and release, and finds kicks, snares and hats.
 */
export class AudioDynamics {
  private floor = new Float32Array(BANDS)
  private ceiling = new Float32Array(BANDS)
  private smooth = new Float32Array(BANDS)
  private target = new Float32Array(BANDS)
  private previous = new Float32Array(BANDS)
  private peaks = new Float32Array(BANDS)
  private holds = new Float32Array(BANDS)
  private fall = new Float32Array(BANDS)
  private kick = new OnsetDetector(0, 4, 5.5, 0.015, 0.18)
  private snare = new OnsetDetector(6, 12, 7, 0.012, 0.14)
  private hat = new OnsetDetector(12, 16, 12, 0.012, 0.07)
  private short = 0
  private long = 0
  private primed = false
  private lastNow: number | null = null
  private lastMedia: number | null = null
  private clock = 0

  reset(): void {
    Object.assign(this, new AudioDynamics())
  }

  update(sample: VisualizerSample, now: number): VisualizerSignal {
    const dt =
      this.lastNow === null ? 1 / 60 : Math.min(0.1, Math.max(0, (now - this.lastNow) / 1000))
    this.lastNow = now
    const raw = sample.bands
    const first = !this.primed
    if (first) {
      for (let i = 0; i < BANDS; i++) {
        this.floor[i] = Math.max(0, raw[i]! - 0.08)
        this.ceiling[i] = raw[i]! + 0.08
      }
      this.primed = true
    }

    const attack = 1 - Math.exp(-dt / 0.015)
    const release = 1 - Math.exp(-dt / 0.11)
    let energy = 0
    this.previous.set(this.target)
    for (let i = 0; i < BANDS; i++) {
      const value = raw[i]!
      // Fast to widen the range, slow to narrow it, so one quiet bar cannot pump the gain.
      this.ceiling[i] =
        value > this.ceiling[i]! ? value : this.ceiling[i]! - (this.ceiling[i]! - value) * dt * 0.25
      this.floor[i] =
        value < this.floor[i]! ? value : this.floor[i]! + (value - this.floor[i]!) * dt * 0.35
      const span = Math.max(0.1, this.ceiling[i]! - this.floor[i]!)
      const stretched = Math.min(1, Math.max(0, (value - this.floor[i]!) / span))
      const gate = Math.min(1, Math.max(0, (value - 0.02) / 0.08))
      // Square the stretched level so quiet bands fall away and loud bands stand out.
      const target = Math.min(1, value * 0.2 + stretched * stretched * 0.95) * gate
      this.target[i] = target

      const rate = target > this.smooth[i]! ? attack : release
      this.smooth[i] = this.smooth[i]! + (target - this.smooth[i]!) * rate
      energy += this.smooth[i]! / BANDS

      if (this.smooth[i]! >= this.peaks[i]!) {
        this.peaks[i] = this.smooth[i]!
        this.holds[i] = 0.3
        this.fall[i] = 0
      } else if ((this.holds[i] = this.holds[i]! - dt) <= 0) {
        this.fall[i] = this.fall[i]! + dt * 2.4
        this.peaks[i] = Math.max(this.smooth[i]!, this.peaks[i]! - this.fall[i]! * dt)
      }
    }
    if (first) {
      // The first frame has no history. It is not an onset and it sets the loudness baseline.
      this.previous.set(this.target)
      this.short = this.long = energy
    }
    for (const detector of [this.kick, this.snare, this.hat])
      detector.update(this.target, this.previous, dt)

    const levels = LEVEL_RANGES.map(([from, to]) => {
      let sum = 0
      for (let i = from; i < to; i++) sum += this.smooth[i]!
      return sum / (to - from)
    }) as VisualizerSignal['levels']
    this.short += (energy - this.short) * (1 - Math.exp(-dt / 0.8))
    this.long += (energy - this.long) * (1 - Math.exp(-dt / 4))
    const swell = Math.min(1, Math.max(0, 0.5 + (this.short - this.long) * 3))

    // Seeks and pauses must not jump or advance the scene.
    const media = sample.timeSeconds
    const step = this.lastMedia === null ? 0 : media - this.lastMedia
    this.lastMedia = media
    if (step > 0 && step < 0.25)
      this.clock +=
        step * Math.max(0.15, 0.35 + energy * 1.2 + this.kick.pulse * 1.1 + (swell - 0.5))

    return {
      bands: this.smooth.slice(),
      peaks: this.peaks.slice(),
      levels,
      hits: [this.kick.pulse, this.snare.pulse, this.hat.pulse],
      beat: this.kick.pulse,
      swell,
      timeSeconds: this.clock,
    }
  }
}
