import { invoke } from '@tauri-apps/api/core'
import { expect, it, vi } from 'vitest'
import { playbackApi, type SmartPlaylistDefinition } from '@/api'
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
export const definition: SmartPlaylistDefinition = {
  match: 'all',
  rules: [{ field: 'playCount', operator: 'equals', value: 0 }],
  sort: { field: 'libraryOrder', direction: 'asc' },
  limit: null,
}
it('uses exact preview and freeze contracts without a playback mutation', async () => {
  const preview = { totalMatches: 1, matches: [{ trackId: 'a', matchedRuleIndexes: [0] }] }
  vi.mocked(invoke)
    .mockResolvedValueOnce(preview)
    .mockResolvedValueOnce({ tracks: [], playlists: [] })
  await expect(playbackApi.previewSmartPlaylist(definition)).resolves.toEqual(preview)
  await playbackApi.freezeSmartPlaylist('smart')
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ['preview_smart_playlist', { definition }],
    ['freeze_smart_playlist', { id: 'smart' }],
  ])
})
