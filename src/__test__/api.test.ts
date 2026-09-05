import { invoke } from '@tauri-apps/api/core'
import { beforeEach, describe, expect, expectTypeOf, it, vi } from 'vitest'

import {
  playbackApi,
  type EditableTrackMetadata,
  type ImportProgress,
  type LibrarySnapshot,
  type PlaybackSnapshot,
  type Playlist,
  windowApi,
  youtubeAuthApi,
} from '../api'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const snapshot: PlaybackSnapshot = {
  status: 'paused',
  currentItem: null,
  positionMs: 0,
  volumePercent: 72,
  queue: [],
}

describe('playbackApi', () => {
  it('inspects the active import with no arguments and a nullable result', async () => {
    expectTypeOf(playbackApi.inspectImportProgress).returns.toEqualTypeOf<
      Promise<ImportProgress | null>
    >()
    vi.mocked(invoke).mockResolvedValue(null)
    await expect(playbackApi.inspectImportProgress()).resolves.toBeNull()
    expect(invoke).toHaveBeenCalledWith('inspect_import_progress')
  })
  it('uses the agreed hardening command names and arguments', async () => {
    await playbackApi.searchYouTube('session')
    expect(invoke).toHaveBeenLastCalledWith('search_youtube', {
      query: 'session',
    })
    await playbackApi.cancelYouTubeImport(7)
    expect(invoke).toHaveBeenLastCalledWith('cancel_youtube_import', {
      runId: 7,
    })
    await playbackApi.retryMetadataRefreshes()
    expect(invoke).toHaveBeenLastCalledWith('retry_metadata_refreshes')
    await playbackApi.clearQueue()
    expect(invoke).toHaveBeenLastCalledWith('clear_queue')
    await playbackApi.resetTrackMetadata(['a'])
    expect(invoke).toHaveBeenLastCalledWith('reset_track_metadata', {
      ids: ['a'],
    })
    await playbackApi.inspectDiagnostics()
    expect(invoke).toHaveBeenLastCalledWith('inspect_diagnostics')
    await playbackApi.exportLibraryBackup()
    expect(invoke).toHaveBeenLastCalledWith('export_library_backup')
  })
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
  })

  it('requests the playback snapshot from the inspect command', async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot)

    await expect(playbackApi.inspect()).resolves.toEqual(snapshot)

    expect(invoke).toHaveBeenCalledWith('inspect_playback')
  })

  it('starts many YouTube sources through the background import service', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined)
    const urls = [
      'https://www.youtube.com/playlist?list=PL-example',
      'https://www.youtube.com/@artist/videos',
    ]

    await expect(playbackApi.importYouTubeUrls(urls)).resolves.toBeUndefined()

    expect(invoke).toHaveBeenCalledWith('import_youtube_urls', { urls })
  })

  it('uses a payload-free response for live volume updates', () => {
    expectTypeOf(playbackApi.setVolume).returns.toEqualTypeOf<Promise<void>>()
  })

  it('plays a selected library track through the playback service', async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot)

    await expect(playbackApi.playTrack('M7lc1UVf-VE', ['M7lc1UVf-VE'])).resolves.toEqual(snapshot)

    expect(invoke).toHaveBeenCalledWith('play_track', {
      id: 'M7lc1UVf-VE',
      queueIds: ['M7lc1UVf-VE'],
    })
  })

  it('adds a selected library track to the front or end of the play queue', async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot)

    await playbackApi.playNext('M7lc1UVf-VE')
    await playbackApi.addToQueue('BaW_jenozKc')

    expect(invoke).toHaveBeenNthCalledWith(1, 'queue_track_next', {
      id: 'M7lc1UVf-VE',
    })
    expect(invoke).toHaveBeenNthCalledWith(2, 'add_to_queue', {
      id: 'BaW_jenozKc',
    })
  })

  it('changes shuffle and repeat modes through dedicated playback commands', async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot)

    await playbackApi.toggleShuffle()
    await playbackApi.cycleRepeatMode()

    expect(vi.mocked(invoke).mock.calls).toEqual([['toggle_shuffle'], ['cycle_repeat_mode']])
  })

  it('removes a library track and an upcoming queue item through dedicated commands', async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot)

    await playbackApi.removeTracks(['BaW_jenozKc'])
    await playbackApi.removeQueueItem(1)

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ['remove_tracks', { ids: ['BaW_jenozKc'] }],
      ['remove_queue_item', { index: 1 }],
    ])
  })

  it('updates selected library metadata through the batched command', async () => {
    const library: LibrarySnapshot = { playlists: [], tracks: [] }
    const updates: Array<{ id: string; metadata: EditableTrackMetadata }> = [
      {
        id: 'M7lc1UVf-VE',
        metadata: {
          album: 'API Sessions',
          artist: 'Google for Developers',
          genres: ['Educational'],
          label: null,
          title: 'YouTube Developers Live',
        },
      },
    ]
    vi.mocked(invoke).mockResolvedValue(library)

    await expect(playbackApi.updateTracksMetadata(updates)).resolves.toEqual(library)

    expect(invoke).toHaveBeenCalledWith('update_tracks_metadata', { updates })
  })

  it('toggles a track in the durable Favorites playlist', async () => {
    const library: LibrarySnapshot = { playlists: [], tracks: [] }
    vi.mocked(invoke).mockResolvedValue(library)

    await expect(playbackApi.toggleFavorite('BaW_jenozKc')).resolves.toEqual(library)

    expect(invoke).toHaveBeenCalledWith('toggle_favorite', {
      id: 'BaW_jenozKc',
    })
  })

  it('creates, updates, and deletes a user playlist through durable commands', async () => {
    const library: LibrarySnapshot = { playlists: [], tracks: [] }
    const playlist: Playlist = {
      id: 'focus',
      name: 'Focus',
      trackIds: ['M7lc1UVf-VE'],
    }
    vi.mocked(invoke).mockResolvedValue(library)

    await playbackApi.upsertPlaylist(playlist)
    await playbackApi.deletePlaylist(playlist.id)

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ['upsert_playlist', { playlist }],
      ['delete_playlist', { id: 'focus' }],
    ])
  })

  it('reorders user playlists through a durable command', async () => {
    const library: LibrarySnapshot = { playlists: [], tracks: [] }
    vi.mocked(invoke).mockResolvedValue(library)

    await expect(playbackApi.reorderPlaylists(['road-trip', 'focus'])).resolves.toEqual(library)

    expect(invoke).toHaveBeenCalledWith('reorder_playlists', {
      playlistIds: ['road-trip', 'focus'],
    })
  })
})

describe('youtubeAuthApi', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset()
  })

  it('uses isolated commands for the YouTube account session', async () => {
    vi.mocked(invoke).mockResolvedValue({ connected: false })

    await youtubeAuthApi.inspect()
    await youtubeAuthApi.openLogin()
    await youtubeAuthApi.saveSession()
    await youtubeAuthApi.disconnect()

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ['inspect_youtube_auth'],
      ['open_youtube_login'],
      ['save_youtube_session'],
      ['disconnect_youtube'],
    ])
  })
})

describe('windowApi', () => {
  it('opens the dedicated Import Music window', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined)

    await expect(windowApi.showImport()).resolves.toBeUndefined()

    expect(invoke).toHaveBeenCalledWith('show_import_window')
  })
})
