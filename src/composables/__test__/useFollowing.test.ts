import { flushPromises } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'

import type { FollowingSnapshot } from '@/api'
import { useFollowing } from '../useFollowing'

const channel = 'UCMYSjwig4-XrzqY6Ka28t4g'
const empty: FollowingSnapshot = { artists: [] }
const followed: FollowingSnapshot = {
  artists: [
    {
      channelId: channel,
      name: 'Lemuria',
      followedAtMs: 1,
      baselineEstablished: false,
      lastCheckedAtMs: null,
      lastError: null,
      checking: false,
      uploads: [],
    },
  ],
}

function client(overrides = {}) {
  return {
    inspect: vi.fn().mockResolvedValue(empty),
    follow: vi.fn().mockResolvedValue(followed),
    unfollow: vi.fn().mockResolvedValue(empty),
    check: vi.fn().mockResolvedValue(followed),
    ...overrides,
  }
}

describe('useFollowing', () => {
  it('loads the snapshot without starting a network check', async () => {
    const api = client()
    const following = useFollowing(api)
    await following.refresh()
    expect(following.snapshot.value).toEqual(empty)
    expect(api.check).not.toHaveBeenCalled()
  })

  it('tracks pending channels while following and clears them afterwards', async () => {
    let finish!: (value: FollowingSnapshot) => void
    const api = client({ follow: vi.fn(() => new Promise((resolve) => (finish = resolve))) })
    const following = useFollowing(api)
    const request = following.follow(channel, 'Lemuria')
    expect(following.pendingChannelIds.value).toEqual([channel])
    finish(followed)
    await request
    expect(following.pendingChannelIds.value).toEqual([])
    expect(following.snapshot.value).toEqual(followed)
  })

  it('ignores a repeated follow while one is pending', async () => {
    const api = client({ follow: vi.fn(() => new Promise(() => {})) })
    const following = useFollowing(api)
    void following.follow(channel, 'Lemuria')
    void following.follow(channel, 'Lemuria')
    expect(api.follow).toHaveBeenCalledTimes(1)
  })

  it('shows a visible error and keeps the snapshot when a command fails', async () => {
    const api = client({
      unfollow: vi.fn().mockRejectedValue({ code: 'x', message: 'Storage failed.' }),
    })
    const following = useFollowing(api)
    await following.refresh()
    await following.follow(channel, 'Lemuria')
    await following.unfollow(channel)
    expect(following.errorMessage.value).toBe('Storage failed.')
    expect(following.snapshot.value).toEqual(followed)
    expect(following.pendingChannelIds.value).toEqual([])
  })

  it('marks the manual check as running and rejects overlap', async () => {
    let finish!: (value: FollowingSnapshot) => void
    const api = client({ check: vi.fn(() => new Promise((resolve) => (finish = resolve))) })
    const following = useFollowing(api)
    const first = following.check()
    expect(following.isChecking.value).toBe(true)
    void following.check()
    expect(api.check).toHaveBeenCalledTimes(1)
    finish(followed)
    await first
    expect(following.isChecking.value).toBe(false)
  })

  it('keeps the latest snapshot when a refresh fails', async () => {
    const api = client()
    const following = useFollowing(api)
    await following.refresh()
    api.inspect.mockRejectedValueOnce(new Error('offline'))
    await following.refresh()
    expect(following.snapshot.value).toEqual(empty)
    expect(following.errorMessage.value).toBe('offline')
  })

  describe('stale responses', () => {
    const other = 'UC_x5XG1OV2P6uZZ5FSM9Ttw'
    const withArtists = (...ids: string[]): FollowingSnapshot => ({
      artists: ids.map((channelId) => ({ ...followed.artists[0], channelId, name: channelId })),
    })
    function deferred<T>() {
      let resolve!: (value: T) => void
      let reject!: (reason: unknown) => void
      const promise = new Promise<T>((res, rej) => {
        resolve = res
        reject = rej
      })
      return { promise, resolve, reject }
    }

    it('does not let a slow initial inspect erase a successful follow', async () => {
      const slow = deferred<FollowingSnapshot>()
      const api = client({
        inspect: vi.fn().mockReturnValueOnce(slow.promise).mockResolvedValue(followed),
      })
      const following = useFollowing(api)
      const initial = following.refresh()
      await following.follow(channel, 'Lemuria')
      slow.resolve(empty)
      await initial
      await flushPromises()
      expect(following.snapshot.value).toEqual(followed)
    })

    it('does not let a late check resurrect an unfollowed artist', async () => {
      const lateCheck = deferred<FollowingSnapshot>()
      const api = client({
        inspect: vi.fn().mockResolvedValueOnce(followed).mockResolvedValue(empty),
        check: vi.fn(() => lateCheck.promise),
      })
      const following = useFollowing(api)
      await following.refresh()
      const check = following.check()
      // Unfollow stays possible while the network check runs.
      await following.unfollow(channel)
      expect(following.isChecking.value).toBe(true)
      expect(following.snapshot.value).toEqual(empty)
      lateCheck.resolve(followed)
      await check
      await flushPromises()
      expect(following.snapshot.value).toEqual(empty)
      expect(following.isChecking.value).toBe(false)
    })

    it('keeps both writes when concurrent follows answer out of order', async () => {
      const first = deferred<FollowingSnapshot>()
      const second = deferred<FollowingSnapshot>()
      const both = withArtists(channel, other)
      const api = client({
        follow: vi.fn().mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise),
        inspect: vi.fn().mockResolvedValue(both),
      })
      const following = useFollowing(api)
      const one = following.follow(channel, 'One')
      const two = following.follow(other, 'Two')
      second.resolve(both)
      await two
      first.resolve(withArtists(channel))
      await one
      await flushPromises()
      expect(following.snapshot.value).toEqual(both)
      expect(following.pendingChannelIds.value).toEqual([])
    })

    it('ignores an obsolete read error after a newer success', async () => {
      const slow = deferred<FollowingSnapshot>()
      const api = client({
        inspect: vi.fn().mockReturnValueOnce(slow.promise).mockResolvedValue(followed),
      })
      const following = useFollowing(api)
      const initial = following.refresh()
      await following.follow(channel, 'Lemuria')
      slow.reject(new Error('old failure'))
      await initial
      await flushPromises()
      expect(following.errorMessage.value).toBe('')
      expect(following.snapshot.value).toEqual(followed)
    })

    it('ignores an obsolete mutation error after a newer success', async () => {
      const failing = deferred<FollowingSnapshot>()
      const api = client({
        follow: vi
          .fn()
          .mockReturnValueOnce(failing.promise)
          .mockResolvedValueOnce(withArtists(other)),
        inspect: vi.fn().mockResolvedValue(withArtists(other)),
      })
      const following = useFollowing(api)
      const slow = following.follow(channel, 'One')
      await following.follow(other, 'Two')
      failing.reject(new Error('old failure'))
      await slow
      await flushPromises()
      expect(following.errorMessage.value).toBe('')
    })

    it('coalesces overlapping refreshes', async () => {
      const slow = deferred<FollowingSnapshot>()
      const api = client({ inspect: vi.fn().mockReturnValue(slow.promise) })
      const following = useFollowing(api)
      const reads = [following.refresh(), following.refresh(), following.refresh()]
      slow.resolve(followed)
      await Promise.all(reads)
      expect(api.inspect.mock.calls.length).toBeLessThanOrEqual(2)
      expect(following.snapshot.value).toEqual(followed)
    })

    it('applies nothing and starts no reads after the owning scope stops', async () => {
      const pending = deferred<FollowingSnapshot>()
      const api = client({ follow: vi.fn(() => pending.promise) })
      const scope = effectScope()
      const following = scope.run(() => useFollowing(api))!
      const request = following.follow(channel, 'Lemuria')
      scope.stop()
      pending.resolve(followed)
      await request
      await flushPromises()
      expect(following.snapshot.value).toEqual(empty)
      expect(api.inspect).not.toHaveBeenCalled()
      await following.refresh()
      expect(api.inspect).not.toHaveBeenCalled()
    })

    it('ignores a failure that arrives after the owning scope stops', async () => {
      const pending = deferred<FollowingSnapshot>()
      const api = client({ inspect: vi.fn(() => pending.promise) })
      const scope = effectScope()
      const following = scope.run(() => useFollowing(api))!
      const request = following.refresh()
      scope.stop()
      pending.reject(new Error('late'))
      await request
      expect(following.errorMessage.value).toBe('')
    })
  })

  describe('retry', () => {
    it('reports an initial load failure and retries the read', async () => {
      const api = client({
        inspect: vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue(followed),
      })
      const following = useFollowing(api)
      await following.refresh()
      expect(following.isLoaded.value).toBe(false)
      expect(following.errorMessage.value).toBe('offline')
      await following.retry()
      expect(following.isLoaded.value).toBe(true)
      expect(following.errorMessage.value).toBe('')
      expect(following.snapshot.value).toEqual(followed)
    })

    it('retries the failed follow with the same arguments', async () => {
      const api = client({
        follow: vi
          .fn()
          .mockRejectedValueOnce(new Error('Storage failed.'))
          .mockResolvedValue(followed),
      })
      const following = useFollowing(api)
      await following.follow(channel, 'Lemuria')
      await following.retry()
      expect(api.follow).toHaveBeenNthCalledWith(2, channel, 'Lemuria')
      expect(following.errorMessage.value).toBe('')
      expect(following.snapshot.value).toEqual(followed)
    })

    it('clears the error without a retry', async () => {
      const api = client({ follow: vi.fn().mockRejectedValue(new Error('x')) })
      const following = useFollowing(api)
      await following.follow(channel, 'Lemuria')
      following.clearError()
      expect(following.errorMessage.value).toBe('')
    })
  })
})
