import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { playbackApi, type MediaItem } from '@/api'

import ImportWindow from '../ImportWindow.vue'

vi.mock('@/api', () => ({ playbackApi: { searchYouTube: vi.fn() } }))

const result: MediaItem = {
  id: 'video',
  title: 'Search title',
  artist: 'Artist',
  album: 'Album',
  durationMs: 125000,
  sourceUrl: 'https://youtube.com/watch?v=video',
}

describe('ImportWindow', () => {
  beforeEach(() => vi.mocked(playbackApi.searchYouTube).mockReset())
  it('puts link entry first and keeps activity outside the form scroll area', () => {
    const wrapper = mount(ImportWindow, {
      props: {
        isImporting: false,
        progress: {
          completedSources: 0,
          importedTracks: 456,
          message: 'Discovering tracks.',
          phase: 'resolving',
          runId: 10,
          skippedMemberOnly: 0,
          totalSources: 1,
        },
      },
    })
    expect(wrapper.findAll('form')[0].attributes('aria-label')).toBe('Import music from YouTube')
    const activity = wrapper.get('[aria-label="Import progress"]')
    expect(activity.element.closest('[data-slot="scroll-area-viewport"]')).toBeNull()
    expect(activity.text()).toContain('456 tracks found')
    expect(wrapper.get('[data-import-status]').text()).toBe('Discovering tracks')
    expect(wrapper.get('progress').attributes('aria-label')).toBe('Import progress')
    expect(wrapper.get('progress').attributes('value')).toBeUndefined()
    expect(wrapper.get('textarea').attributes('rows')).toBe('3')
  })

  it('does not show an empty terminal before the first import', () => {
    const wrapper = mount(ImportWindow, { props: { isImporting: false } })
    expect(wrapper.find('[role="log"]').exists()).toBe(false)
    expect(wrapper.find('progress').exists()).toBe(false)
    expect(wrapper.text()).toContain('Imports run in the background')
  })

  it.each(['started', 'merging'] as const)(
    'shows indeterminate %s activity instead of a stalled zero bar',
    (phase) => {
      const wrapper = mount(ImportWindow, {
        props: {
          isImporting: true,
          progress: {
            completedSources: 0,
            importedTracks: 12,
            message: 'Working',
            phase,
            runId: 2,
            skippedMemberOnly: 0,
            totalSources: 1,
          },
        },
      })
      expect(wrapper.get('progress').attributes('value')).toBeUndefined()
    },
  )

  it("clears the previous run's log when reopening on a later phase", async () => {
    const progress = {
      completedSources: 1,
      importedTracks: 1,
      message: 'Old run',
      phase: 'completed' as const,
      runId: 1,
      skippedMemberOnly: 0,
      totalSources: 1,
    }
    const wrapper = mount(ImportWindow, {
      props: { isImporting: false, progress },
    })
    await wrapper.setProps({
      progress: {
        ...progress,
        runId: 2,
        phase: 'resolving',
        message: 'New run',
      },
    })
    expect(wrapper.get('[role="log"]').text()).toBe('New run')
    expect(wrapper.get('[data-import-log]').classes()).toContain('h-24')
  })

  it('does not clear a failed URL draft when a separate search import completes', async () => {
    vi.mocked(playbackApi.searchYouTube).mockResolvedValue([result])
    const wrapper = mount(ImportWindow, { props: { isImporting: false } })
    const input = wrapper.get('textarea[aria-label="YouTube URLs"]')
    await input.setValue('https://youtu.be/keep-draft')
    await wrapper.get('form[aria-label="Import music from YouTube"]').trigger('submit')
    await wrapper.get('input[aria-label="Search YouTube"]').setValue('song')
    await wrapper.get('form[aria-label="Search YouTube"]').trigger('submit')
    await flushPromises()
    await wrapper.get('[data-search-result] button').trigger('click')
    await wrapper.setProps({
      progress: {
        completedSources: 1,
        importedTracks: 1,
        message: 'Complete',
        phase: 'completed',
        runId: 9,
        skippedMemberOnly: 0,
        totalSources: 1,
      },
    })
    expect((input.element as HTMLTextAreaElement).value).toBe('https://youtu.be/keep-draft')
  })
  it('keeps failed URL drafts and retries the same import', async () => {
    const wrapper = mount(ImportWindow, { props: { isImporting: false } })
    const input = wrapper.get('textarea[aria-label="YouTube URLs"]')
    await input.setValue('https://youtu.be/failed')
    await wrapper.get('form[aria-label="Import music from YouTube"]').trigger('submit')
    await wrapper.setProps({ errorMessage: 'Import failed. Check the URL.' })
    expect((input.element as HTMLTextAreaElement).value).toBe('https://youtu.be/failed')
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Retry')!
      .trigger('click')
    expect(wrapper.emitted('importYoutubeUrls')).toEqual([
      [['https://youtu.be/failed']],
      [['https://youtu.be/failed']],
    ])
  })

  it('retries the explicitly selected search import after backend failure', async () => {
    vi.mocked(playbackApi.searchYouTube).mockResolvedValue([result])
    const wrapper = mount(ImportWindow, { props: { isImporting: false } })
    await wrapper.get('input[aria-label="Search YouTube"]').setValue('song')
    await wrapper.get('form[aria-label="Search YouTube"]').trigger('submit')
    await flushPromises()
    await wrapper.get('[data-search-result] button').trigger('click')
    await wrapper.setProps({ errorMessage: 'Import failed' })
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Retry')!
      .trigger('click')
    expect(wrapper.emitted('importYoutubeUrls')).toEqual([
      [[result.sourceUrl]],
      [[result.sourceUrl]],
    ])
  })
  it('searches text and previews bounded results without importing until requested', async () => {
    vi.mocked(playbackApi.searchYouTube).mockResolvedValue(
      Array.from({ length: 25 }, (_, i) => ({ ...result, id: `video${i}` })),
    )
    const wrapper = mount(ImportWindow, { props: { isImporting: false } })
    await wrapper.get('input[aria-label="Search YouTube"]').setValue('  jazz session  ')
    await wrapper.get('form[aria-label="Search YouTube"]').trigger('submit')
    await flushPromises()
    expect(playbackApi.searchYouTube).toHaveBeenCalledWith('jazz session')
    expect(wrapper.findAll('[data-search-result]')).toHaveLength(20)
    expect(wrapper.text()).toContain('Artist')
    expect(wrapper.text()).toContain('Album')
    expect(wrapper.emitted('importYoutubeUrls')).toBeUndefined()
    await wrapper.get('[data-search-result] button').trigger('click')
    expect(wrapper.emitted('importYoutubeUrls')).toEqual([[[result.sourceUrl]]])
  })

  it('discards stale search responses and exposes a retry after errors', async () => {
    let finish!: (items: MediaItem[]) => void
    vi.mocked(playbackApi.searchYouTube)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          finish = resolve
        }),
      )
      .mockResolvedValueOnce([{ ...result, title: 'Latest result' }])
      .mockRejectedValueOnce({ message: 'Search unavailable' })
      .mockResolvedValueOnce([])
    const wrapper = mount(ImportWindow, { props: { isImporting: false } })
    const input = wrapper.get('input[aria-label="Search YouTube"]')
    const form = wrapper.get('form[aria-label="Search YouTube"]')
    await input.setValue('old')
    await form.trigger('submit')
    await input.setValue('new')
    await form.trigger('submit')
    await flushPromises()
    finish([result])
    await flushPromises()
    expect(wrapper.text()).toContain('Latest result')
    expect(wrapper.text()).not.toContain('Search title')
    await input.setValue('third')
    await form.trigger('submit')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('Search unavailable')
    await wrapper.get('button[aria-label="Retry search"]').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('No results')
  })

  it('cancels only the active run and keeps committed results visible', async () => {
    const progress = {
      completedSources: 1,
      importedTracks: 3,
      message: 'Importing',
      phase: 'resolving' as const,
      runId: 7,
      skippedMemberOnly: 0,
      totalSources: 2,
    }
    const wrapper = mount(ImportWindow, {
      props: { isImporting: true, progress },
    })
    await wrapper.get('button[aria-label="Cancel import"]').trigger('click')
    expect(wrapper.emitted('cancelImport')).toEqual([[7]])
    await wrapper.setProps({
      isImporting: false,
      progress: {
        ...progress,
        phase: 'cancelled',
        message: 'Import cancelled',
      },
    })
    expect(wrapper.text()).toContain('3 tracks remain in your library')
    expect(wrapper.find('button[aria-label="Cancel import"]').exists()).toBe(false)
  })
  it('owns the Import Music window landmark', () => {
    const wrapper = mount(ImportWindow, {
      props: { isImporting: false },
    })

    expect(wrapper.get('main').attributes('aria-label')).toBe('Import music')
    expect(wrapper.get('h1').text()).toBe('Import Music')
  })

  it('submits many unique video, playlist, and artist URLs', async () => {
    const wrapper = mount(ImportWindow, {
      props: { isImporting: false },
    })
    const input = wrapper.get('textarea[aria-label="YouTube URLs"]')

    await input.setValue(
      [
        'https://youtu.be/M7lc1UVf-VE',
        'https://youtube.com/playlist?list=PL-example',
        'https://youtube.com/@artist/videos',
        'https://youtu.be/M7lc1UVf-VE',
      ].join('\n'),
    )
    await wrapper.get('form[aria-label="Import music from YouTube"]').trigger('submit')

    expect(wrapper.emitted('importYoutubeUrls')).toEqual([
      [
        [
          'https://youtu.be/M7lc1UVf-VE',
          'https://youtube.com/playlist?list=PL-example',
          'https://youtube.com/@artist/videos',
        ],
      ],
    ])
    expect(input.attributes('placeholder')).toContain('one URL per line')
    expect((input.element as HTMLTextAreaElement).value).toContain('https://youtu.be/M7lc1UVf-VE')
  })

  it('shows import errors in the Import Music window', () => {
    const wrapper = mount(ImportWindow, {
      props: {
        errorMessage: 'could not resolve YouTube metadata',
        isImporting: false,
      },
    })

    expect(wrapper.get('[role="alert"]').text()).toBe('could not resolve YouTube metadata')
  })

  it('shows import progress and terminal output', async () => {
    const wrapper = mount(ImportWindow, {
      props: {
        isImporting: true,
        progress: {
          completedSources: 1,
          importedTracks: 12,
          message: 'Resolved source 1 of 2; 12 track(s) found.',
          phase: 'resolving',
          runId: 4,
          skippedMemberOnly: 3,
          totalSources: 2,
        },
      },
    })

    await wrapper.vm.$nextTick()

    expect(wrapper.get('progress').attributes('value')).toBe('50')
    expect(wrapper.get('[role="log"]').text()).toContain(
      'Resolved source 1 of 2; 12 track(s) found.',
    )
    expect(wrapper.findAll("[data-slot='scroll-area-viewport']")).toHaveLength(2)
    expect(wrapper.text()).toContain('3 members-only tracks skipped')
    expect(wrapper.get('textarea').attributes('disabled')).toBeDefined()
  })

  it('shows indeterminate progress while the first source streams track metadata', () => {
    const wrapper = mount(ImportWindow, {
      props: {
        isImporting: true,
        progress: {
          completedSources: 0,
          importedTracks: 12,
          message: 'Found 12 track(s) while reading source 1 of 1…',
          phase: 'resolving',
          runId: 5,
          skippedMemberOnly: 0,
          totalSources: 1,
        },
      },
    })

    expect(wrapper.get('progress').attributes('value')).toBeUndefined()
    expect(wrapper.text()).toContain('12 tracks found')
  })
})
