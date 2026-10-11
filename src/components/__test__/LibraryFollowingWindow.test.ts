import { DOMWrapper, enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'

import type { FollowingSnapshot, MediaItem } from '@/api'
import LibraryWindow from '../LibraryWindow.vue'

enableAutoUnmount(afterEach)

const channel = 'UCMYSjwig4-XrzqY6Ka28t4g'
const tracks: MediaItem[] = [
  {
    id: 'inlibrary11',
    title: 'Owned',
    artist: 'Lemuria',
    durationMs: 1000,
    channelId: channel,
    channel: 'Lemuria',
  },
]
const following: FollowingSnapshot = {
  artists: [
    {
      channelId: channel,
      name: 'Lemuria',
      followedAtMs: 1,
      baselineEstablished: true,
      lastCheckedAtMs: 5,
      lastError: null,
      checking: false,
      uploads: [
        {
          videoId: 'newnewnew11',
          title: 'Fresh Song',
          durationMs: 1000,
          url: 'https://www.youtube.com/watch?v=newnewnew11',
          discoveredAtMs: 5,
          isNew: true,
        },
      ],
    },
  ],
}

const mountWindow = (props: Record<string, unknown> = {}) =>
  mount(LibraryWindow, {
    attachTo: document.body,
    props: { isUpdating: false, tracks, following, ...props },
  })

describe('LibraryWindow following', () => {
  it('shows followed artists and imports one upload only on request', async () => {
    const wrapper = mountWindow()
    await wrapper.get('[data-collection="following"]').trigger('click')
    expect(wrapper.get('[data-followed-artist]').text()).toContain('Lemuria')
    expect(wrapper.emitted('importUrls')).toBeUndefined()
    await wrapper.get('[aria-label="Add Fresh Song to library"]').trigger('click')
    expect(wrapper.emitted('importUrls')).toEqual([
      [['https://www.youtube.com/watch?v=newnewnew11']],
    ])
  })

  it('forwards manual check, per-artist check, and unfollow', async () => {
    const wrapper = mountWindow()
    await wrapper.get('[data-collection="following"]').trigger('click')
    await wrapper.get('button[data-check-following]').trigger('click')
    await wrapper.get('[aria-label="Check Lemuria for new uploads"]').trigger('click')
    await wrapper.get('[aria-label="Unfollow Lemuria"]').trigger('click')
    expect(wrapper.emitted('checkFollowing')).toEqual([[], [channel]])
    expect(wrapper.emitted('unfollowArtist')).toEqual([[channel]])
  })

  it('follows an artist from the Artists view context menu', async () => {
    const wrapper = mountWindow({ following: { artists: [] } })
    await wrapper.get('[data-collection="artists"]').trigger('click')
    await wrapper.get('.artist-tile').trigger('contextmenu')
    await flushPromises()
    const menu = new DOMWrapper(
      document.body.querySelector('[data-artist-context-menu]') as HTMLElement,
    )
    await menu
      .findAll('[role="menuitem"]')
      .find((node) => node.text() === 'Follow artist')!
      .trigger('click')
    expect(wrapper.emitted('followArtist')).toEqual([[{ channelId: channel, name: 'Lemuria' }]])
  })

  it('keeps followed artists listed when no library tracks remain', async () => {
    const wrapper = mountWindow({ tracks: [] })
    await wrapper.get('[data-collection="following"]').trigger('click')
    expect(wrapper.get('[data-followed-artist]').text()).toContain('Lemuria')
  })

  it('forwards Retry loading after a failed load', async () => {
    const wrapper = mountWindow({
      following: { artists: [] },
      followingLoadFailed: true,
      followingError: 'offline',
    })
    await wrapper.get('[data-collection="following"]').trigger('click')
    await wrapper.get('button[data-retry-following]').trigger('click')
    expect(wrapper.emitted('retryFollowing')).toEqual([[]])
  })
})
