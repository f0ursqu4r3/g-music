import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import YouTubeArtwork from '../YouTubeArtwork.vue'

const artworkMocks = vi.hoisted(() => ({
  resolveYouTube: vi.fn(),
  invalidateYouTube: vi.fn(),
}))

vi.mock('@/api', () => ({
  artworkApi: artworkMocks,
}))

describe('YouTubeArtwork', () => {
  beforeEach(() => {
    artworkMocks.resolveYouTube.mockReset()
    artworkMocks.invalidateYouTube.mockReset()
  })

  it('uses the application artwork cache', async () => {
    artworkMocks.resolveYouTube.mockResolvedValue('asset://localhost/cached-artwork.jpg')
    const wrapper = mount(YouTubeArtwork, {
      props: { videoId: 'M7lc1UVf-VE' },
    })
    await flushPromises()

    expect(wrapper.get('img').attributes('src')).toBe('asset://localhost/cached-artwork.jpg')
    expect(artworkMocks.resolveYouTube).toHaveBeenCalledWith('M7lc1UVf-VE')
  })

  it('leaves the local artwork fallback visible when cached artwork is broken', async () => {
    artworkMocks.resolveYouTube.mockResolvedValue('asset://localhost/broken.jpg')
    const wrapper = mount(YouTubeArtwork, {
      props: { videoId: 'M7lc1UVf-VE' },
    })
    await flushPromises()

    await wrapper.get('img').trigger('error')

    expect(artworkMocks.invalidateYouTube).toHaveBeenCalledWith('M7lc1UVf-VE')
    expect(artworkMocks.resolveYouTube).toHaveBeenCalledTimes(1)
    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.get('[data-artwork-placeholder]').attributes('aria-hidden')).toBe('true')
  })

  it('ignores a stale response after the video changes', async () => {
    let finish!: (path: string) => void
    artworkMocks.resolveYouTube.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve
      }),
    )
    const wrapper = mount(YouTubeArtwork, { props: { videoId: 'M7lc1UVf-VE' } })
    artworkMocks.resolveYouTube.mockResolvedValue('asset://new.jpg')
    await wrapper.setProps({ videoId: 'BaW_jenozKc' })
    await flushPromises()
    finish('asset://old.jpg')
    await flushPromises()
    expect(wrapper.get('img').attributes('src')).toBe('asset://new.jpg')
  })

  it('ignores an error from an image replaced by a newer video', async () => {
    artworkMocks.resolveYouTube.mockResolvedValueOnce('asset://old.jpg')
    const wrapper = mount(YouTubeArtwork, { props: { videoId: 'M7lc1UVf-VE' } })
    await flushPromises()
    const oldImage = wrapper.get('img')
    artworkMocks.resolveYouTube.mockResolvedValueOnce('asset://new.jpg')
    await wrapper.setProps({ videoId: 'BaW_jenozKc' })
    await flushPromises()
    await oldImage.trigger('error')
    expect(wrapper.get('img').attributes('src')).toBe('asset://new.jpg')
    expect(artworkMocks.invalidateYouTube).not.toHaveBeenCalled()
  })

  it('rejects padded IDs', async () => {
    mount(YouTubeArtwork, { props: { videoId: ' M7lc1UVf-VE ' } })
    await flushPromises()
    expect(artworkMocks.resolveYouTube).not.toHaveBeenCalled()
  })

  it('renders the artwork fallback when no video artwork can be requested', () => {
    const wrapper = mount(YouTubeArtwork, {
      props: { videoId: 'track-1' },
    })

    expect(wrapper.find('[data-artwork-placeholder]').exists()).toBe(true)
    expect(wrapper.find('img').exists()).toBe(false)
  })

  it('does not request YouTube artwork for non-video identifiers', () => {
    const wrapper = mount(YouTubeArtwork, {
      props: { videoId: 'track-1' },
    })

    expect(wrapper.find('img').exists()).toBe(false)
    expect(artworkMocks.resolveYouTube).not.toHaveBeenCalled()
  })
})
