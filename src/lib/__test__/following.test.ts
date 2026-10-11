import { describe, expect, it } from 'vitest'

import type { MediaItem } from '@/api'
import { artistChannels, isChannelId } from '../following'

const track = (patch: Partial<MediaItem>): MediaItem => ({
  id: 'aaaaaaaaaaa',
  title: 'Song',
  artist: 'Lemuria',
  durationMs: 1000,
  ...patch,
})

describe('isChannelId', () => {
  it('accepts stable channel IDs only', () => {
    expect(isChannelId('UCMYSjwig4-XrzqY6Ka28t4g')).toBe(true)
    for (const bad of ['@lemurian.archives', 'Lemuria', 'UCshort', '', null, undefined]) {
      expect(isChannelId(bad)).toBe(false)
    }
  })
})

describe('artistChannels', () => {
  it('collects distinct valid channel IDs with names and counts', () => {
    const channels = artistChannels([
      track({ channelId: 'UCMYSjwig4-XrzqY6Ka28t4g', channel: 'Lemuria' }),
      track({ id: 'bbbbbbbbbbb', channelId: 'UCMYSjwig4-XrzqY6Ka28t4g', channel: 'Lemuria' }),
      track({ id: 'ccccccccccc', channelId: 'UC_x5XG1OV2P6uZZ5FSM9Ttw', channel: 'Other' }),
    ])
    expect(channels).toEqual([
      { channelId: 'UCMYSjwig4-XrzqY6Ka28t4g', name: 'Lemuria', trackCount: 2 },
      { channelId: 'UC_x5XG1OV2P6uZZ5FSM9Ttw', name: 'Other', trackCount: 1 },
    ])
  })

  it('never treats an uploader handle or missing ID as a channel', () => {
    expect(
      artistChannels([
        track({ channelId: null, uploaderId: '@lemurian.archives' }),
        track({ id: 'bbbbbbbbbbb', channelId: '@lemurian.archives' }),
        track({ id: 'ccccccccccc' }),
      ]),
    ).toEqual([])
  })

  it('falls back to the uploader then artist name for display', () => {
    expect(
      artistChannels([
        track({ channelId: 'UCMYSjwig4-XrzqY6Ka28t4g', uploader: 'Uploader' }),
        track({ id: 'b', channelId: 'UC_x5XG1OV2P6uZZ5FSM9Ttw' }),
      ]).map((channel) => channel.name),
    ).toEqual(['Uploader', 'Lemuria'])
  })
})
