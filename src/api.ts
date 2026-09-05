import { convertFileSrc, invoke } from '@tauri-apps/api/core'

export interface MediaItem {
  id: string
  provider?: string
  sourceUrl?: string | null
  title: string
  artist: string
  album?: string | null
  albumArtist?: string | null
  trackNumber?: number | null
  discNumber?: number | null
  releaseDate?: string | null
  uploadDate?: string | null
  description?: string | null
  channel?: string | null
  channelId?: string | null
  uploader?: string | null
  uploaderId?: string | null
  thumbnailUrl?: string | null
  label?: string | null
  genres?: string[]
  categories?: string[]
  tags?: string[]
  language?: string | null
  availability?: string | null
  isLive?: boolean
  viewCount?: number | null
  likeCount?: number | null
  durationMs: number
  metadataDirty?: boolean
  playCount?: number
  lastPlayedAtMs?: number | null
  playHistoryMs?: number[]
}

export type PlaybackStatus = 'paused' | 'playing'
export type RepeatMode = 'off' | 'all' | 'one'

export interface PlaybackSnapshot {
  status: PlaybackStatus
  currentItem: MediaItem | null
  positionMs: number
  volumePercent: number
  shuffleEnabled?: boolean
  repeatMode?: RepeatMode
  queue: MediaItem[]
  playbackOrder?: string[]
}

export interface PlaybackTransport {
  status: PlaybackStatus
  currentItem: MediaItem | null
  positionMs: number
  volumePercent: number
  shuffleEnabled?: boolean
  repeatMode?: RepeatMode
}

export interface LibrarySnapshot {
  playlists: Playlist[]
  tracks: MediaItem[]
}

export interface Playlist {
  id: string
  name: string
  trackIds: string[]
}

export interface EditableTrackMetadata {
  title: string
  artist: string
  album: string | null
  label: string | null
  genres: string[]
}

export interface TrackMetadataUpdate {
  id: string
  metadata: Partial<EditableTrackMetadata>
}

export type ImportProgressPhase =
  'started' | 'resolving' | 'merging' | 'completed' | 'failed' | 'cancelled'

export interface ImportProgress {
  completedSources: number
  importedTracks: number
  message: string
  phase: ImportProgressPhase
  runId: number
  skippedMemberOnly: number
  totalSources: number
}

export type MetadataRefreshState = 'queued' | 'refreshing' | 'completed' | 'failed' | 'skipped'

export interface MetadataRefreshJob {
  message: string
  state: MetadataRefreshState
  trackId: string
  title: string
}

export interface MetadataRefreshSnapshot {
  completedTracks: number
  jobs: MetadataRefreshJob[]
  totalTracks: number
}

export interface CommandError {
  code: string
  message: string
}

export interface YouTubeAuthStatus {
  connected: boolean
}

export interface DiagnosticsSnapshot {
  appVersion: string
  platform: string
  dependencies: {
    name: string
    available: boolean
    version: string | null
    message: string
  }[]
  audioOutputPolicy: string
}

export const playbackApi = {
  searchYouTube: (query: string): Promise<MediaItem[]> => invoke('search_youtube', { query }),
  cancelYouTubeImport: (runId: number): Promise<void> => invoke('cancel_youtube_import', { runId }),
  retryMetadataRefreshes: (): Promise<MetadataRefreshSnapshot> =>
    invoke('retry_metadata_refreshes'),
  clearQueue: (): Promise<PlaybackSnapshot> => invoke('clear_queue'),
  resetTrackMetadata: (ids: string[]): Promise<LibrarySnapshot> =>
    invoke('reset_track_metadata', { ids }),
  inspectDiagnostics: (): Promise<DiagnosticsSnapshot> => invoke('inspect_diagnostics'),
  exportLibraryBackup: (): Promise<{ path: string }> => invoke('export_library_backup'),
  inspect: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>('inspect_playback'),
  inspectLibrary: (): Promise<LibrarySnapshot> => invoke<LibrarySnapshot>('inspect_library'),
  inspectTransport: (): Promise<PlaybackTransport> =>
    invoke<PlaybackTransport>('inspect_playback_transport'),
  inspectMetadataRefreshes: (): Promise<MetadataRefreshSnapshot> =>
    invoke<MetadataRefreshSnapshot>('inspect_metadata_refreshes'),
  inspectImportProgress: (): Promise<ImportProgress | null> =>
    invoke<ImportProgress | null>('inspect_import_progress'),
  updateTracksMetadata: (updates: TrackMetadataUpdate[]): Promise<LibrarySnapshot> =>
    invoke<LibrarySnapshot>('update_tracks_metadata', { updates }),
  toggleFavorite: (id: string): Promise<LibrarySnapshot> =>
    invoke<LibrarySnapshot>('toggle_favorite', { id }),
  removeTracks: (ids: string[]): Promise<LibrarySnapshot> =>
    invoke<LibrarySnapshot>('remove_tracks', { ids }),
  upsertPlaylist: (playlist: Playlist): Promise<LibrarySnapshot> =>
    invoke<LibrarySnapshot>('upsert_playlist', { playlist }),
  reorderPlaylists: (playlistIds: string[]): Promise<LibrarySnapshot> =>
    invoke<LibrarySnapshot>('reorder_playlists', { playlistIds }),
  deletePlaylist: (id: string): Promise<LibrarySnapshot> =>
    invoke<LibrarySnapshot>('delete_playlist', { id }),
  importYouTubeUrls: (urls: string[]): Promise<void> =>
    invoke<void>('import_youtube_urls', { urls }),
  play: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>('play'),
  pause: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>('pause'),
  playTrack: (id: string, queueIds?: string[]): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>('play_track', {
      id,
      ...(queueIds ? { queueIds } : {}),
    }),
  playNext: (id: string): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>('queue_track_next', { id }),
  addToQueue: (id: string): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>('add_to_queue', { id }),
  previous: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>('previous'),
  next: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>('next'),
  toggleShuffle: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>('toggle_shuffle'),
  cycleRepeatMode: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>('cycle_repeat_mode'),
  seek: (positionMs: number): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>('seek', { positionMs }),
  setVolume: (volumePercent: number): Promise<void> =>
    invoke<void>('set_volume', { volumePercent }),
  moveQueueItem: (from: number, to: number): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>('move_queue_item', { from, to }),
  removeQueueItem: (index: number): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>('remove_queue_item', { index }),
}

export const artworkApi = {
  resolveYouTube: async (videoId: string): Promise<string | null> => {
    const localPath = await invoke<string | null>('resolve_youtube_artwork', {
      videoId,
    })
    return localPath ? convertFileSrc(localPath) : null
  },
}

export const windowApi = {
  showImport: (): Promise<void> => invoke<void>('show_import_window'),
}

export const youtubeAuthApi = {
  inspect: (): Promise<YouTubeAuthStatus> => invoke<YouTubeAuthStatus>('inspect_youtube_auth'),
  openLogin: (): Promise<YouTubeAuthStatus> => invoke<YouTubeAuthStatus>('open_youtube_login'),
  saveSession: (): Promise<YouTubeAuthStatus> => invoke<YouTubeAuthStatus>('save_youtube_session'),
  disconnect: (): Promise<YouTubeAuthStatus> => invoke<YouTubeAuthStatus>('disconnect_youtube'),
}
