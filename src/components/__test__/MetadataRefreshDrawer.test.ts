import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import MetadataRefreshDrawer from '../MetadataRefreshDrawer.vue'

describe('MetadataRefreshDrawer', () => {
  it('shows real completion progress, puts active work first, and collapses completed jobs', async () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          totalTracks: 4,
          completedTracks: 2,
          jobs: [
            {
              trackId: 'done',
              title: 'Saved track',
              state: 'completed',
              message: 'Full metadata saved to the library.',
            },
            {
              trackId: 'queued',
              title: 'Waiting track',
              state: 'queued',
              message: 'Waiting for metadata refresh.',
            },
            {
              trackId: 'failed',
              title: 'Failed track',
              state: 'failed',
              message: 'Network unavailable',
            },
            {
              trackId: 'active',
              title: 'Current track',
              state: 'refreshing',
              message: 'Fetching full YouTube metadata.',
            },
          ],
        },
      },
    })
    expect(wrapper.get('progress').attributes('value')).toBe('2')
    expect(wrapper.get('progress').attributes('max')).toBe('4')
    expect(wrapper.get('[data-refresh-summary]').text()).toContain('1 refreshed')
    expect(wrapper.get('[data-refresh-summary]').text()).toContain('1 failed')
    expect(wrapper.findAll('[data-refresh-track-id]')[0].attributes('data-refresh-track-id')).toBe(
      'active',
    )
    expect(wrapper.get('[data-refresh-group="completed"]').attributes('open')).toBeUndefined()
    expect(wrapper.text()).not.toContain('Waiting for metadata refresh.')
    expect(wrapper.text()).not.toContain('Full metadata saved to the library.')
    expect(wrapper.get('aside').classes()).toContain('bg-(--popover)')
    await wrapper.get('button[aria-label="Close metadata refresh"]').trigger('click')
    expect(wrapper.emitted('close')).toHaveLength(1)
  })

  it('offers failed jobs a retry and shows retry errors without losing jobs', async () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          completedTracks: 0,
          totalTracks: 1,
          jobs: [
            {
              trackId: 'one',
              title: 'Failed track',
              state: 'failed',
              message: 'Network unavailable',
            },
          ],
        },
        errorMessage: 'Retry failed. Check your connection.',
      },
    })
    expect(wrapper.text()).not.toContain('Retry on restart')
    await wrapper.get('button[aria-label="Retry failed metadata"]').trigger('click')
    expect(wrapper.emitted('retry')).toHaveLength(1)
    expect(wrapper.get('[role="alert"]').text()).toContain('Check your connection')
    expect(wrapper.text()).toContain('Failed track')
    await wrapper.setProps({ isRetrying: true })
    expect(
      wrapper.get('button[aria-label="Retry failed metadata"]').attributes('disabled'),
    ).toBeDefined()
  })
  it('skipped jobs appear in a dedicated Skipped section, not in refreshed or failed counts', () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          totalTracks: 3,
          completedTracks: 3,
          jobs: [
            {
              trackId: 'done',
              title: 'Saved track',
              state: 'completed',
              message: 'Full metadata saved to the library.',
            },
            {
              trackId: 'skip1',
              title: 'Supporter-only track',
              state: 'skipped',
              message:
                'Subscriber-only content is unavailable to this YouTube session. Hidden from the library and play queue.',
            },
            {
              trackId: 'skip2',
              title: 'Another locked track',
              state: 'skipped',
              message:
                'Subscriber-only content is unavailable to this YouTube session. Hidden from the library and play queue.',
            },
          ],
        },
      },
    })
    // skipped tracks must appear in their own group
    expect(wrapper.find('[data-refresh-group="skipped"]').exists()).toBe(true)
    // skipped are NOT counted as refreshed or failed
    expect(wrapper.get('[data-refresh-summary]').text()).toContain('1 refreshed')
    expect(wrapper.get('[data-refresh-summary]').text()).not.toContain('failed')
    // skipped count shown
    expect(wrapper.get('[data-refresh-summary]').text()).toContain('2 skipped')
    // individual tracks are listed in the skipped group
    expect(wrapper.get('[data-refresh-track-id="skip1"]').text()).toContain('Supporter-only track')
  })

  it('does not show Retry failed button when only skipped jobs are present (no real failures)', () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          totalTracks: 1,
          completedTracks: 1,
          jobs: [
            {
              trackId: 'skip1',
              title: 'Supporter track',
              state: 'skipped',
              message:
                'Subscriber-only content is unavailable to this YouTube session. Hidden from the library and play queue.',
            },
          ],
        },
      },
    })
    expect(wrapper.find('button[aria-label="Retry failed metadata"]').exists()).toBe(false)
  })

  it('shows Retry failed only when real failed jobs exist alongside skipped', () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          totalTracks: 2,
          completedTracks: 2,
          jobs: [
            {
              trackId: 'fail1',
              title: 'Network failed',
              state: 'failed',
              message: 'Network unavailable',
            },
            {
              trackId: 'skip1',
              title: 'Supporter track',
              state: 'skipped',
              message:
                'Subscriber-only content is unavailable to this YouTube session. Hidden from the library and play queue.',
            },
          ],
        },
      },
    })
    expect(wrapper.find('button[aria-label="Retry failed metadata"]').exists()).toBe(true)
  })

  it("header status text says 'Finished with errors' when only real failures, not when only skipped", () => {
    const wrapperSkipOnly = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          totalTracks: 1,
          completedTracks: 1,
          jobs: [
            {
              trackId: 'skip1',
              title: 'Supporter track',
              state: 'skipped',
              message:
                'Subscriber-only content is unavailable to this YouTube session. Hidden from the library and play queue.',
            },
          ],
        },
      },
    })
    expect(wrapperSkipOnly.get('[role="status"]').text()).not.toContain('Finished with errors')
    expect(wrapperSkipOnly.get('[role="status"]').text()).toContain('Finished with skipped tracks')
    expect(wrapperSkipOnly.get('progress').attributes('aria-valuetext')).toContain('1 skipped')
    expect(wrapperSkipOnly.get('[data-refresh-group="skipped"] summary').text()).toContain(
      'Skipped',
    )
  })

  it('fills the bounded popover so its job list can scroll', () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          completedTracks: 0,
          jobs: [],
          totalTracks: 0,
        },
      },
    })

    expect(wrapper.get('[aria-label="Metadata refreshes"]').classes()).toEqual(
      expect.arrayContaining(['h-full', 'min-h-0']),
    )
  })

  it('shows queued and active dirty-track refreshes', () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          completedTracks: 3,
          jobs: [
            {
              message: 'Waiting for metadata refresh.',
              state: 'queued',
              trackId: 'M7lc1UVf-VE',
              title: 'Discovered track',
            },
            {
              message: 'Fetching full YouTube metadata.',
              state: 'refreshing',
              trackId: 'BaW_jenozKc',
              title: 'Playing track',
            },
          ],
          totalTracks: 5,
        },
      },
    })

    expect(wrapper.get('[aria-label="Metadata refreshes"]').text()).toContain('3 refreshed')
    expect(wrapper.get('[data-refresh-track-id="M7lc1UVf-VE"]').text()).toContain('Queued')
    expect(wrapper.get('[data-refresh-track-id="BaW_jenozKc"]').text()).toContain('Refreshing')
    expect(wrapper.find("[data-slot='scroll-area-viewport']").exists()).toBe(true)
  })
})
