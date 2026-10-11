import type { MediaItem } from '@/api'
import type { ArtistChannel } from '@/components/library/types'

const CHANNEL_ID = /^UC[A-Za-z0-9_-]{22}$/

export function isChannelId(value: string | null | undefined): value is string {
  return typeof value === 'string' && CHANNEL_ID.test(value)
}

/** Distinct YouTube channels behind tracks. Only validated channel IDs count; handles never do. */
export function artistChannels(tracks: Iterable<MediaItem>): ArtistChannel[] {
  const channels = new Map<string, ArtistChannel>()
  for (const track of tracks) {
    if (!isChannelId(track.channelId)) continue
    const channel = channels.get(track.channelId)
    if (channel) channel.trackCount += 1
    else
      channels.set(track.channelId, {
        channelId: track.channelId,
        name: track.channel?.trim() || track.uploader?.trim() || track.artist,
        trackCount: 1,
      })
  }
  return [...channels.values()]
}
