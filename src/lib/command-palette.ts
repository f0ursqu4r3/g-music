import type { MediaItem, Playlist } from '@/api'
import { ordinaryPlaylist } from './smart-playlists'
export const COMMAND_RESULT_LIMIT = 40
export type CommandMode = 'play' | 'next' | 'queue'
export interface CommandResult {
  id: string
  title: string
  detail: string
  kind: 'track' | 'album' | 'artist' | 'playlist' | 'action' | 'selection'
  trackIds: string[]
  playlistId?: string
}
export const paletteActions = [
  ['new-playlist', 'New playlist'],
  ['new-smart-playlist', 'New smart playlist'],
  ['import', 'Import music'],
  ['queue', 'Open Queue'],
  ['settings', 'Open Settings'],
] as const
export function searchCommands(
  query: string,
  tracks: MediaItem[],
  playlists: Playlist[],
  selectedIds: string[] = [],
): CommandResult[] {
  const results: CommandResult[] = []
  const albums = new Map<string, CommandResult>()
  const artists = new Map<string, CommandResult>()
  const available = new Set(tracks.map((track) => track.id))
  for (const track of tracks) {
    results.push({
      id: `track:${track.id}`,
      title: track.title,
      detail: [track.artist, track.album].filter(Boolean).join(' · '),
      kind: 'track',
      trackIds: [track.id],
    })
    if (track.album?.trim()) {
      const key = `${track.artist}\u0000${track.album.trim()}`
      const album = albums.get(key) ?? {
        id: `album:${key}`,
        title: track.album.trim(),
        detail: track.artist,
        kind: 'album',
        trackIds: [],
      }
      album.trackIds.push(track.id)
      albums.set(key, album)
    }
    const artist = artists.get(track.artist) ?? {
      id: `artist:${track.artist}`,
      title: track.artist,
      detail: 'Artist',
      kind: 'artist',
      trackIds: [],
    }
    artist.trackIds.push(track.id)
    artists.set(track.artist, artist)
  }
  results.push(...albums.values(), ...artists.values())
  for (const playlist of playlists) {
    results.push({
      id: `playlist:${playlist.id}`,
      title: playlist.name,
      detail: playlist.smart ? 'Smart playlist' : 'Playlist',
      kind: 'playlist',
      trackIds: playlist.trackIds.filter((id) => available.has(id)),
      playlistId: playlist.id,
    })
    if (selectedIds.length && ordinaryPlaylist(playlist))
      results.push({
        id: `selection:${playlist.id}`,
        title: `Add selection to ${playlist.name}`,
        detail: `${selectedIds.length} selected tracks`,
        kind: 'selection',
        trackIds: [...selectedIds],
        playlistId: playlist.id,
      })
  }
  const actions: CommandResult[] = paletteActions.map(([id, title]) => ({
    id,
    title,
    kind: 'action',
    detail: 'Action',
    trackIds: [],
  }))
  const normalized = query.trim().toLocaleLowerCase()
  // Keep actions discoverable in an empty query, even in a large library.
  const candidates = normalized
    ? [...results, ...actions]
    : [
        ...actions,
        ...results.filter((r) => r.kind === 'selection'),
        ...results.filter((r) => r.kind !== 'selection'),
      ]
  const terms = normalized.split(/\s+/).filter(Boolean)
  return candidates
    .map((result, order) => {
      const title = result.title.toLocaleLowerCase()
      const text = `${title} ${result.detail.toLocaleLowerCase()}`
      const rank = !normalized
        ? 0
        : title === normalized
          ? 0
          : title.startsWith(normalized)
            ? 1
            : title.includes(normalized)
              ? 2
              : terms.every((term) => text.includes(term))
                ? 3
                : 4
      return { result, order, rank }
    })
    .filter((entry) => entry.rank < 4)
    .sort((a, b) => a.rank - b.rank || a.order - b.order)
    .slice(0, COMMAND_RESULT_LIMIT)
    .map((entry) => entry.result)
}
export function isPaletteShortcut(event: KeyboardEvent): boolean {
  return (
    !event.defaultPrevented &&
    !event.repeat &&
    !event.isComposing &&
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'k'
  )
}
export function hasOpenOverlay(): boolean {
  return Boolean(
    document.querySelector(
      '[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"], [role="menu"][data-state="open"]',
    ),
  )
}
