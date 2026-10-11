import { describe, expect, it } from 'vitest'
import { AudioDynamics } from '../dynamics'

const flat = (value: number) => new Float32Array(16).fill(value)
function run(dynamics: AudioDynamics, frames: number, bands: (frame: number) => Float32Array) {
  let signal = dynamics.update({ bands: bands(0), timeSeconds: 0 }, 0)
  for (let frame = 1; frame <= frames; frame++)
    signal = dynamics.update({ bands: bands(frame), timeSeconds: frame / 60 }, (frame * 1000) / 60)
  return signal
}

describe('AudioDynamics', () => {
  it('stays dark for silence', () => {
    const signal = run(new AudioDynamics(), 120, () => flat(0))
    expect(Math.max(...signal.bands)).toBe(0)
    expect(signal.beat).toBe(0)
  })

  it('expands a compressed loudness range', () => {
    // RMS levels arrive between 0.6 and 0.75 for typical mastered music.
    const dynamics = new AudioDynamics()
    let low = 1,
      high = 0
    for (let frame = 0; frame < 600; frame++) {
      const signal = dynamics.update(
        { bands: flat(frame % 60 < 30 ? 0.6 : 0.75), timeSeconds: frame / 60 },
        (frame * 1000) / 60,
      )
      if (frame >= 540) {
        low = Math.min(low, signal.bands[0]!)
        high = Math.max(high, signal.bands[0]!)
      }
    }
    expect(high - low).toBeGreaterThan(0.5)
  })

  it('detects kicks and lets the pulse decay', () => {
    const dynamics = new AudioDynamics()
    const kick = (frame: number) => {
      const bands = flat(0.55)
      if (frame % 30 < 3) bands.fill(0.85, 0, 4)
      return bands
    }
    let beats = 0
    let previous = 0
    let signal = dynamics.update({ bands: kick(0), timeSeconds: 0 }, 0)
    for (let frame = 1; frame < 600; frame++) {
      signal = dynamics.update({ bands: kick(frame), timeSeconds: frame / 60 }, (frame * 1000) / 60)
      if (signal.beat > 0.99 && previous < 0.9) beats++
      previous = signal.beat
    }
    expect(beats).toBeGreaterThanOrEqual(15)
    expect(signal.beat).toBeLessThan(0.3)
  })

  it('separates kicks, snares and hats', () => {
    const count = (pattern: (frame: number, bands: Float32Array) => void) => {
      const dynamics = new AudioDynamics()
      const totals = [0, 0, 0]
      let previous = [0, 0, 0]
      for (let frame = 0; frame < 600; frame++) {
        const bands = flat(0.55)
        pattern(frame, bands)
        const { hits } = dynamics.update({ bands, timeSeconds: frame / 60 }, (frame * 1000) / 60)
        hits.forEach((hit, i) => {
          if (hit > 0.99 && previous[i]! < 0.9) totals[i]!++
        })
        previous = [...hits]
      }
      return totals
    }
    const [kickOnly, snareFromKick, hatFromKick] = count((frame, bands) => {
      if (frame % 30 < 3) bands.fill(0.85, 0, 4)
    })
    const [kickFromSnare, snareOnly] = count((frame, bands) => {
      if ((frame + 15) % 30 < 3) bands.fill(0.85, 6, 12)
    })
    const [kickFromHat, , hatOnly] = count((frame, bands) => {
      if (frame % 8 < 1) bands.fill(0.85, 12, 16)
    })
    expect(kickOnly).toBeGreaterThanOrEqual(15)
    expect(snareOnly).toBeGreaterThanOrEqual(15)
    expect(hatOnly).toBeGreaterThanOrEqual(50)
    expect(snareFromKick + hatFromKick + kickFromSnare + kickFromHat).toBe(0)
  })

  it('reports a build above steady loudness', () => {
    const dynamics = new AudioDynamics()
    let signal = run(dynamics, 1800, (frame) => flat(0.4 + (frame % 20) * 0.01))
    expect(signal.swell).toBeGreaterThan(0.4)
    expect(signal.swell).toBeLessThan(0.6)
    for (let frame = 1801; frame < 1920; frame++)
      signal = dynamics.update(
        { bands: flat(frame % 4 < 2 ? 0.9 : 0.85), timeSeconds: frame / 60 },
        (frame * 1000) / 60,
      )
    expect(signal.swell).toBeGreaterThan(0.6)
  })

  it('holds peaks above bands and lets them fall', () => {
    const dynamics = new AudioDynamics()
    const loud = run(dynamics, 60, (frame) => flat(frame < 40 ? 0.5 + (frame % 2) * 0.3 : 0.5))
    expect(loud.peaks[3]!).toBeGreaterThanOrEqual(loud.bands[3]!)
    const later = run(dynamics, 240, () => flat(0.5))
    expect(later.peaks[3]!).toBeLessThanOrEqual(loud.peaks[3]!)
  })

  it('moves faster when the music is louder and freezes when paused', () => {
    const quiet = run(new AudioDynamics(), 300, () => flat(0.05))
    const loud = run(new AudioDynamics(), 300, (frame) => flat(0.4 + (frame % 20 < 10 ? 0.4 : 0)))
    expect(loud.timeSeconds).toBeGreaterThan(quiet.timeSeconds)
    const paused = new AudioDynamics()
    const first = paused.update({ bands: flat(0.7), timeSeconds: 5 }, 0)
    const second = paused.update({ bands: flat(0.7), timeSeconds: 5 }, 500)
    expect(second.timeSeconds).toBe(first.timeSeconds)
  })

  it('ignores seeks when advancing motion', () => {
    const dynamics = new AudioDynamics()
    dynamics.update({ bands: flat(0.5), timeSeconds: 10 }, 0)
    const jumped = dynamics.update({ bands: flat(0.5), timeSeconds: 200 }, 16)
    expect(jumped.timeSeconds).toBeLessThan(1)
  })
})
