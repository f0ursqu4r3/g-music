import { DOMWrapper, enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'

import LibraryArtistGrid from '../LibraryArtistGrid.vue'
import LibraryInfoPanel from '../LibraryInfoPanel.vue'
import type { LibraryArtist } from '../types'

enableAutoUnmount(afterEach)

const one = 'UCMYSjwig4-XrzqY6Ka28t4g'
const two = 'UC_x5XG1OV2P6uZZ5FSM9Ttw'

const artist = (channels: LibraryArtist['channels']): LibraryArtist => ({
  albumCount: 1,
  channels,
  detail: '1 song',
  durationMs: 1000,
  name: 'Lemuria',
  trackCount: 1,
  videoId: 'aaaaaaaaaaa',
})

function mountGrid(target: LibraryArtist, displayMode: 'grid' | 'list', followed: string[] = []) {
  return mount(LibraryArtistGrid, {
    attachTo: document.body,
    props: {
      displayMode,
      followedChannelIds: followed,
      followPendingIds: [],
      gridItemSize: 160,
      groups: [{ items: [target], label: '' }],
      selectedArtistName: undefined,
      sortBy: 'artist-asc',
    },
    global: { stubs: { LibraryVirtualGrid: false } },
  })
}

async function openMenu(wrapper: ReturnType<typeof mountGrid>) {
  await wrapper.get('.artist-tile').trigger('contextmenu')
  await new Promise((resolve) => setTimeout(resolve, 0))
  return new DOMWrapper(document.body.querySelector('[data-artist-context-menu]') as HTMLElement)
}

describe.each(['list', 'grid'] as const)('artist context menu (%s)', (mode) => {
  it('follows the single known channel', async () => {
    const wrapper = mountGrid(artist([{ channelId: one, name: 'Lemuria', trackCount: 1 }]), mode)
    const menu = await openMenu(wrapper)
    const item = menu.findAll('[role="menuitem"]').find((node) => node.text() === 'Follow artist')
    expect(item).toBeDefined()
    await item!.trigger('click')
    expect(wrapper.emitted('followChannel')).toEqual([[{ channelId: one, name: 'Lemuria' }]])
  })

  it('unfollows an already followed channel', async () => {
    const wrapper = mountGrid(artist([{ channelId: one, name: 'Lemuria', trackCount: 1 }]), mode, [
      one,
    ])
    const menu = await openMenu(wrapper)
    const item = menu.findAll('[role="menuitem"]').find((node) => node.text() === 'Unfollow artist')
    expect(item).toBeDefined()
    await item!.trigger('click')
    expect(wrapper.emitted('unfollowChannel')).toEqual([[one]])
  })

  it('asks which channel to follow when the artist has several', async () => {
    const wrapper = mountGrid(
      artist([
        { channelId: one, name: 'Lemuria', trackCount: 1 },
        { channelId: two, name: 'Lemuria Archives', trackCount: 1 },
      ]),
      mode,
    )
    const menu = await openMenu(wrapper)
    const labels = menu.findAll('[role="menuitem"]').map((node) => node.text())
    expect(labels).toContain('Follow channel Lemuria')
    expect(labels).toContain('Follow channel Lemuria Archives')
    expect(labels).not.toContain('Follow artist')
    await menu
      .findAll('[role="menuitem"]')
      .find((node) => node.text() === 'Follow channel Lemuria Archives')!
      .trigger('click')
    expect(wrapper.emitted('followChannel')).toEqual([
      [{ channelId: two, name: 'Lemuria Archives' }],
    ])
  })

  it('disables following and says how to fix missing channel metadata', async () => {
    const wrapper = mountGrid(artist([]), mode)
    const menu = await openMenu(wrapper)
    const item = menu
      .findAll('[role="menuitem"]')
      .find((node) => node.text().startsWith('Follow unavailable'))
    expect(item).toBeDefined()
    expect(item!.attributes('data-disabled')).toBeDefined()
    expect(item!.text()).toContain('refresh metadata')
  })
})

describe('artist info panel following', () => {
  const mountPanel = (target: LibraryArtist, props: Record<string, unknown> = {}) =>
    mount(LibraryInfoPanel, {
      props: {
        isOpen: true,
        selectedAlbum: null,
        selectedArtist: target,
        selectedTrack: null,
        followedChannelIds: [],
        followPendingIds: [],
        ...props,
      },
    })

  it('follows and unfollows with accessible buttons', async () => {
    const target = artist([{ channelId: one, name: 'Lemuria', trackCount: 1 }])
    const wrapper = mountPanel(target)
    await wrapper.get('[aria-label="Follow Lemuria"]').trigger('click')
    expect(wrapper.emitted('followChannel')).toEqual([[{ channelId: one, name: 'Lemuria' }]])

    const followed = mountPanel(target, { followedChannelIds: [one] })
    await followed.get('[aria-label="Unfollow Lemuria"]').trigger('click')
    expect(followed.emitted('unfollowChannel')).toEqual([[one]])
  })

  it('disables the button while the change is pending', () => {
    const target = artist([{ channelId: one, name: 'Lemuria', trackCount: 1 }])
    const wrapper = mountPanel(target, { followPendingIds: [one] })
    expect(wrapper.get('[aria-label="Follow Lemuria"]').attributes('disabled')).toBeDefined()
  })

  it('lists every channel for an ambiguous artist', () => {
    const target = artist([
      { channelId: one, name: 'Lemuria', trackCount: 1 },
      { channelId: two, name: 'Lemuria Archives', trackCount: 1 },
    ])
    const wrapper = mountPanel(target)
    expect(wrapper.find('[aria-label="Follow Lemuria"]').exists()).toBe(true)
    expect(wrapper.find('[aria-label="Follow Lemuria Archives"]').exists()).toBe(true)
  })

  it('reports a clear action when no channel ID is known', () => {
    const wrapper = mountPanel(artist([]))
    expect(wrapper.text()).toContain('Refresh track metadata to follow')
    expect(wrapper.find('[aria-label^="Follow "]').exists()).toBe(false)
  })

  it('offers follow for the channel of a selected track', async () => {
    const wrapper = mount(LibraryInfoPanel, {
      props: {
        isOpen: true,
        selectedAlbum: null,
        selectedArtist: null,
        selectedTrack: {
          id: 'aaaaaaaaaaa',
          title: 'Song',
          artist: 'Lemuria',
          durationMs: 1,
          channelId: one,
          channel: 'Lemuria Archives',
        },
        followedChannelIds: [],
        followPendingIds: [],
      },
    })
    await wrapper.get('[aria-label="Follow Lemuria Archives"]').trigger('click')
    expect(wrapper.emitted('followChannel')).toEqual([
      [{ channelId: one, name: 'Lemuria Archives' }],
    ])
  })
})
