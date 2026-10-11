import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import type { FollowedArtist } from '@/api'
import LibraryFollowing from '../LibraryFollowing.vue'

const channel = 'UCMYSjwig4-XrzqY6Ka28t4g'

const artist = (patch: Partial<FollowedArtist> = {}): FollowedArtist => ({
  channelId: channel,
  name: 'Lemuria',
  followedAtMs: 1,
  baselineEstablished: true,
  lastCheckedAtMs: 1_700_000_000_000,
  lastError: null,
  checking: false,
  uploads: [
    {
      videoId: 'newnewnew11',
      title: 'Fresh Song',
      durationMs: 120_000,
      url: 'https://www.youtube.com/watch?v=newnewnew11',
      discoveredAtMs: 2,
      isNew: true,
    },
    {
      videoId: 'oldoldold11',
      title: 'Earlier Song',
      durationMs: null,
      url: 'https://www.youtube.com/watch?v=oldoldold11',
      discoveredAtMs: 1,
      isNew: false,
    },
  ],
  ...patch,
})

const mountFollowing = (props: Record<string, unknown> = {}) =>
  mount(LibraryFollowing, {
    props: {
      artists: [artist()],
      libraryTrackIds: [],
      pendingChannelIds: [],
      isChecking: false,
      ...props,
    },
  })

describe('LibraryFollowing', () => {
  it('tells new uploads apart from recent uploads that existed before following', () => {
    const wrapper = mountFollowing()
    expect(wrapper.get('[data-upload="newnewnew11"]').text()).toContain('New since you followed')
    expect(wrapper.get('[data-upload="oldoldold11"]').text()).toContain('Recent upload')
    expect(wrapper.get('[data-upload="oldoldold11"]').text()).not.toContain('New since')
  })

  it('explains that the first check only sets a baseline', () => {
    const wrapper = mountFollowing({
      artists: [artist({ baselineEstablished: false, lastCheckedAtMs: null, uploads: [] })],
    })
    expect(wrapper.text()).toContain('Not checked yet')
    expect(wrapper.text()).toContain('first check')
  })

  it('emits an import request for one upload using its stable URL', async () => {
    const wrapper = mountFollowing()
    await wrapper.get('[aria-label="Add Fresh Song to library"]').trigger('click')
    expect(wrapper.emitted('addToLibrary')).toEqual([
      [['https://www.youtube.com/watch?v=newnewnew11']],
    ])
  })

  it('shows uploads already in the library as imported and not importable', () => {
    const wrapper = mountFollowing({ libraryTrackIds: ['newnewnew11'] })
    const row = wrapper.get('[data-upload="newnewnew11"]')
    expect(row.text()).toContain('In library')
    expect(row.find('button').exists()).toBe(false)
  })

  it('disables import while an import runs', () => {
    const wrapper = mountFollowing({ isImporting: true })
    expect(
      wrapper.get('[aria-label="Add Fresh Song to library"]').attributes('disabled'),
    ).toBeDefined()
  })

  it('offers a manual check that disables while checking', async () => {
    const wrapper = mountFollowing()
    await wrapper.get('button[data-check-following]').trigger('click')
    expect(wrapper.emitted('check')).toEqual([[]])

    const busy = mountFollowing({ isChecking: true })
    const button = busy.get('button[data-check-following]')
    expect(button.attributes('disabled')).toBeDefined()
    expect(button.text()).toContain('Checking')
  })

  it('shows a per-artist loading state and error while keeping earlier uploads', () => {
    const wrapper = mountFollowing({
      artists: [artist({ checking: true, lastError: 'Dependency timed out.' })],
    })
    const section = wrapper.get('[data-followed-artist]')
    expect(section.text()).toContain('Checking')
    expect(section.get('[role="alert"]').text()).toContain('Dependency timed out.')
    expect(section.findAll('[data-upload]')).toHaveLength(2)
  })

  it('emits unfollow and per-artist check, disabling both while pending', async () => {
    const wrapper = mountFollowing()
    await wrapper.get('[aria-label="Unfollow Lemuria"]').trigger('click')
    expect(wrapper.emitted('unfollow')).toEqual([[channel]])
    await wrapper.get('[aria-label="Check Lemuria for new uploads"]').trigger('click')
    expect(wrapper.emitted('checkArtist')).toEqual([[channel]])

    const pending = mountFollowing({ pendingChannelIds: [channel] })
    expect(pending.get('[aria-label="Unfollow Lemuria"]').attributes('disabled')).toBeDefined()
  })

  it('shows an empty state and a visible error', () => {
    const wrapper = mountFollowing({ artists: [], errorMessage: 'Storage failed.' })
    expect(wrapper.text()).toContain('not following any artists')
    expect(wrapper.get('[role="alert"]').text()).toContain('Storage failed.')
  })

  it('offers Retry loading when the first load failed and does not claim zero follows', async () => {
    const wrapper = mountFollowing({
      artists: [],
      errorMessage: 'offline',
      loadFailed: true,
    })
    expect(wrapper.text()).not.toContain('not following any artists')
    expect(wrapper.text()).toContain('could not be loaded')
    await wrapper.get('button[data-retry-following]').trigger('click')
    expect(wrapper.emitted('retryLoad')).toEqual([[]])
  })

  it('disables Retry loading while loading', () => {
    const wrapper = mountFollowing({ artists: [], loadFailed: true, isLoading: true })
    expect(wrapper.get('button[data-retry-following]').attributes('disabled')).toBeDefined()
  })

  it('hides Retry loading when the list loaded', () => {
    expect(mountFollowing().find('button[data-retry-following]').exists()).toBe(false)
  })
})
