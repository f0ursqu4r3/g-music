import { DOMWrapper, flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import SmartPlaylistEditor from '../SmartPlaylistEditor.vue'
import { smartPresets } from '@/lib/smart-playlists'
import type { SmartPlaylistPreview } from '@/api'
const body = () => new DOMWrapper(document.body)
let wrapper: VueWrapper
const previewAction = vi.fn()
const saveAction = vi.fn()
async function open() {
  previewAction.mockReset().mockResolvedValue({ totalMatches: 0, matches: [] })
  saveAction.mockReset().mockResolvedValue(undefined)
  wrapper = mount(SmartPlaylistEditor, {
    attachTo: document.body,
    props: { tracks: [], previewAction, saveAction },
  })
  await flushPromises()
}
afterEach(() => {
  wrapper?.unmount()
  document.body.innerHTML = ''
})
it('focuses the name, fills each preset without writes, and sends exact definitions', async () => {
  await open()
  expect(document.activeElement).toBe(body().get('[data-smart-name]').element)
  for (const preset of smartPresets) {
    await body().get(`[data-smart-preset="${preset.id}"]`).trigger('click')
    await body().get('[data-smart-preview]').trigger('click')
    await flushPromises()
    expect(previewAction).toHaveBeenLastCalledWith(preset.definition)
  }
  expect(saveAction).not.toHaveBeenCalled()
})
it('validates field/operator/value changes, trimmed text, integers and limits before IPC', async () => {
  await open()
  await body().get('[data-smart-name]').setValue('Draft')
  await body().get('[aria-label="Rule 1 field"]').setValue('title')
  await body().get('[aria-label="Rule 1 value"]').setValue('   ')
  await body().get('[data-smart-form]').trigger('submit')
  expect(body().get('[role="alert"]').text()).toContain('1 to 200 characters')
  expect(saveAction).not.toHaveBeenCalled()
  await body().get('[aria-label="Rule 1 value"]').setValue('  Jazz  ')
  await body().get('[data-smart-preview]').trigger('click')
  await flushPromises()
  expect(previewAction.mock.calls[0]?.[0].rules[0].value).toBe('Jazz')
  await body().get('[aria-label="Rule 1 field"]').setValue('lastPlayedDays')
  for (const invalid of ['0', '36501', '1.5']) {
    await body().get('[aria-label="Rule 1 value"]').setValue(invalid)
    await body().get('[data-smart-form]').trigger('submit')
    expect(saveAction).not.toHaveBeenCalled()
  }
  await body().get('[aria-label="Rule 1 value"]').setValue('30')
  await body().get('[aria-label="Track limit"]').setValue('10001')
  await body().get('[data-smart-form]').trigger('submit')
  expect(body().get('[role="alert"]').text()).toContain('1 to 10000')
  await body().get('[aria-label="Track limit"]').setValue('')
  await body().get('[data-smart-form]').trigger('submit')
  await flushPromises()
  expect(saveAction).toHaveBeenCalledOnce()
  expect(saveAction.mock.calls[0]?.[0].smart.limit).toBeNull()
})
it('bounds rule count and supports all/any and boolean values', async () => {
  await open()
  await body().get('[aria-label="Rule 1 field"]').setValue('favorite')
  expect(body().get('[aria-label="Rule 1 operator"]').findAll('option')).toHaveLength(1)
  await body().get('[aria-label="Rule 1 value"]').setValue('false')
  await body().get('[aria-label="Match rules"]').setValue('any')
  await body().get('[data-smart-preview]').trigger('click')
  await flushPromises()
  expect(previewAction.mock.calls[0]?.[0]).toMatchObject({
    match: 'any',
    rules: [{ field: 'favorite', operator: 'equals', value: false }],
  })
  const add = body()
    .findAll('button')
    .find((b) => b.text() === 'Add rule')!
  for (let i = 1; i < 20; i++) await add.trigger('click')
  expect(body().findAll('[data-smart-rule]')).toHaveLength(20)
  expect(add.attributes('disabled')).toBeDefined()
  await body().get('[aria-label="Remove rule 2"]').trigger('click')
  expect(body().findAll('[data-smart-rule]')).toHaveLength(19)
})
it('drops stale previews and bounds visible explanations to twenty tracks', async () => {
  await open()
  let resolve!: (value: SmartPlaylistPreview) => void
  previewAction.mockImplementationOnce(
    () =>
      new Promise((r) => {
        resolve = r
      }),
  )
  await body().get('[data-smart-preview]').trigger('click')
  await body().get('[data-smart-preset="short-tracks"]').trigger('click')
  resolve({ totalMatches: 999, matches: [] })
  await flushPromises()
  expect(body().text()).not.toContain('999 matches')
  previewAction.mockResolvedValue({
    totalMatches: 100,
    matches: Array.from({ length: 50 }, (_, i) => ({
      trackId: `Track ${i}`,
      matchedRuleIndexes: [0],
    })),
  })
  await body().get('[data-smart-preview]').trigger('click')
  await flushPromises()
  expect(body().findAll('[data-smart-preview-result]')).toHaveLength(20)
  expect(body().text()).toContain('100 matches before limit · 50 in playlist · showing first 20')
  expect(body().text()).toContain('Duration (milliseconds) is less than 180000')
  await body().get('[aria-label="Sort direction"]').setValue('desc')
  expect(body().findAll('[data-smart-preview-result]')).toHaveLength(0)
})
it('retains a preview failure and allows a retry without saving', async () => {
  await open()
  previewAction.mockRejectedValueOnce(new Error('Preview unavailable'))
  await body().get('[data-smart-preview]').trigger('click')
  await flushPromises()
  expect(body().get('[role="alert"]').text()).toContain('Preview unavailable')
  await body().get('[data-smart-preview]').trigger('click')
  await flushPromises()
  expect(body().text()).toContain('0 matches')
  expect(saveAction).not.toHaveBeenCalled()
})
