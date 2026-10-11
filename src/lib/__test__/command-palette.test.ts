import { expect, it } from 'vitest'
import type { OutputState } from '@/api'
import { searchCommands, COMMAND_RESULT_LIMIT } from '../command-palette'
const tracks = [
  { id: 'substring', title: 'The Echo song', artist: 'Artist', album: 'Sessions', durationMs: 1 },
  { id: 'prefix', title: 'Echoes', artist: 'Artist', album: 'Sessions', durationMs: 1 },
  { id: 'exact', title: 'Echo', artist: 'Artist', album: 'Sessions', durationMs: 1 },
]
it('ranks exact and prefix matches before substrings and preserves collection order', () => {
  expect(searchCommands('echo', tracks, []).map((r) => r.id)).toEqual([
    'track:exact',
    'track:prefix',
    'track:substring',
  ])
  expect(searchCommands('sessions', tracks, [])[0]?.trackIds).toEqual([
    'substring',
    'prefix',
    'exact',
  ])
  expect(
    searchCommands('ordered', tracks, [
      { id: 'p', name: 'Ordered', trackIds: ['exact', 'substring'] },
    ])[0]?.trackIds,
  ).toEqual(['exact', 'substring'])
})
it('bounds results and handles empty libraries and no matches', () => {
  expect(
    searchCommands(
      'song',
      Array.from({ length: 100 }, (_, i) => ({ ...tracks[0]!, id: String(i) })),
      [],
    ),
  ).toHaveLength(COMMAND_RESULT_LIMIT)
  expect(searchCommands('missing', tracks, [])).toEqual([])
  expect(searchCommands('', [], []).some((r) => r.id === 'new-smart-playlist')).toBe(true)
})

const outputState: OutputState = {
  devices: [{ id: 'coreaudio/DAC', name: 'USB DAC' }],
  selectedDeviceId: null,
  airplayAvailable: true,
  airplayReceivers: null,
}

it('offers output devices and AirPlay', () => {
  const results = searchCommands('output', [], [], [], outputState)
  expect(results.map((r) => r.id)).toContain('output:coreaudio/DAC')
  expect(searchCommands('airplay', [], [], [], outputState)[0]).toMatchObject({
    id: 'output:airplay',
    title: 'AirPlay…',
    kind: 'output',
  })
})

it('omits AirPlay when unavailable', () => {
  const results = searchCommands('airplay', [], [], [], { ...outputState, airplayAvailable: false })
  expect(results.find((r) => r.id === 'output:airplay')).toBeUndefined()
})
