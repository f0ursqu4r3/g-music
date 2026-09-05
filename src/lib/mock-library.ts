import trackCatalog from './mock-tracks.json'
import { formatDuration } from './time'

export interface MockTrack {
  id: string
  title: string
  artist: string
  album: string
  duration: string
  cover: string
}

export interface MockAlbum {
  title: string
  artist: string
  cover: string
}

export interface MockArtist {
  name: string
  detail: string
  cover: string
}

export const mockTracks: MockTrack[] = trackCatalog.map(({ durationMs, ...track }) => ({
  ...track,
  duration: formatDuration(durationMs),
}))

export const mockAlbums: MockAlbum[] = [
  { title: 'Afterimage', artist: 'Chromatic Skies', cover: 'violet' },
  { title: 'Drift Pattern', artist: 'Distant Signals', cover: 'cyan' },
  { title: 'Slow Motion', artist: 'Northbound', cover: 'amber' },
  { title: 'Weather System', artist: 'Hollow Coast', cover: 'rose' },
  { title: 'Thin Air', artist: 'Low Season', cover: 'lime' },
]

export const mockArtists: MockArtist[] = [
  { name: 'Chromatic Skies', detail: '1 album · 12 songs', cover: 'violet' },
  { name: 'Distant Signals', detail: '2 albums · 21 songs', cover: 'cyan' },
  { name: 'Northbound', detail: '1 album · 9 songs', cover: 'amber' },
  { name: 'Hollow Coast', detail: '3 albums · 30 songs', cover: 'rose' },
  { name: 'Low Season', detail: '2 albums · 18 songs', cover: 'lime' },
]
