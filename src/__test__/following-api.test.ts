import { invoke } from '@tauri-apps/api/core'
import { expect, it, vi } from 'vitest'

import { followingApi } from '@/api'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

it('uses exact following command contracts', async () => {
  const snapshot = { artists: [] }
  vi.mocked(invoke).mockResolvedValue(snapshot)

  await expect(followingApi.inspect()).resolves.toEqual(snapshot)
  await followingApi.follow('UCMYSjwig4-XrzqY6Ka28t4g', 'Lemuria')
  await followingApi.unfollow('UCMYSjwig4-XrzqY6Ka28t4g')
  await followingApi.check()
  await followingApi.check('UCMYSjwig4-XrzqY6Ka28t4g')

  expect(vi.mocked(invoke).mock.calls).toEqual([
    ['inspect_following'],
    ['follow_artist', { channelId: 'UCMYSjwig4-XrzqY6Ka28t4g', name: 'Lemuria' }],
    ['unfollow_artist', { channelId: 'UCMYSjwig4-XrzqY6Ka28t4g' }],
    ['check_following', { channelId: null }],
    ['check_following', { channelId: 'UCMYSjwig4-XrzqY6Ka28t4g' }],
  ])
})
