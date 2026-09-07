<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import type { MediaItem, Playlist } from '@/api'
import { searchCommands, type CommandMode, type CommandResult } from '@/lib/command-palette'
import LibraryDialog from './library/LibraryDialog.vue'
import { Button } from '@/components/ui/button'
const props = defineProps<{
  tracks: MediaItem[]
  playlists: Playlist[]
  selectedIds?: string[]
  isUpdating?: boolean
  runAction: (result: CommandResult, mode: CommandMode) => Promise<unknown>
}>()
const emit = defineEmits<{ close: [] }>()
const query = ref('')
const mode = ref<CommandMode>('play')
const activeIndex = ref(0)
const busy = ref(false)
const error = ref('')
const results = computed(() =>
  searchCommands(query.value, props.tracks, props.playlists, props.selectedIds),
)
const blocked = computed(() => busy.value || !!props.isUpdating)
const labels = { play: 'Play', next: 'Play next', queue: 'Add to queue' }
let alive = true
onBeforeUnmount(() => {
  alive = false
})
watch(query, () => {
  activeIndex.value = 0
  error.value = ''
})
watch(results, () => {
  activeIndex.value = Math.min(activeIndex.value, Math.max(0, results.value.length - 1))
})
function actionLabel(result: CommandResult): string {
  return result.kind === 'action' || result.kind === 'selection'
    ? result.title
    : `${labels[mode.value]}: ${result.title}`
}
async function run(result: CommandResult): Promise<void> {
  if (blocked.value) return
  if (!['action', 'selection'].includes(result.kind) && !result.trackIds.length) {
    error.value = 'This collection has no available tracks.'
    return
  }
  busy.value = true
  error.value = ''
  try {
    await props.runAction(result, mode.value)
    if (alive) emit('close')
  } catch (cause) {
    if (alive)
      error.value = `${cause instanceof Error ? cause.message : typeof cause === 'object' && cause && 'message' in cause ? String(cause.message) : String(cause)} Try the action again.`
  } finally {
    if (alive) busy.value = false
  }
}
async function keydown(event: KeyboardEvent): Promise<void> {
  if (event.isComposing || event.repeat || event.defaultPrevented) return
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    if (!results.value.length || blocked.value) return
    activeIndex.value =
      (activeIndex.value + (event.key === 'ArrowDown' ? 1 : -1) + results.value.length) %
      results.value.length
    await nextTick()
    document
      .getElementById(`command-option-${activeIndex.value}`)
      ?.scrollIntoView?.({ block: 'nearest' })
  } else if (event.key === 'Enter') {
    event.preventDefault()
    const result = results.value[activeIndex.value]
    if (result) await run(result)
  }
}
</script>
<template>
  <LibraryDialog
    title="Command palette"
    initial-focus="[data-command-search]"
    :busy="busy"
    @close="emit('close')"
  >
    <div class="flex min-h-0 min-w-0 flex-col gap-3">
      <header class="flex items-center justify-between gap-2">
        <h2 class="text-lg font-semibold">Command palette</h2>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          :disabled="busy"
          aria-label="Close command palette"
          @click="emit('close')"
          >Close</Button
        >
      </header>
      <label class="grid gap-1 text-sm"
        >Search music and actions
        <input
          v-model="query"
          data-command-search
          role="combobox"
          aria-autocomplete="list"
          aria-expanded="true"
          aria-controls="command-results"
          :aria-activedescendant="results.length ? `command-option-${activeIndex}` : undefined"
          :disabled="busy"
          autofocus
          class="min-w-0 rounded-md border border-(--line-strong) bg-(--glass-control) px-3 py-2 text-(--text) outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
          @keydown="keydown"
        />
      </label>
      <label class="flex items-center gap-2 text-sm"
        >Music action<select
          v-model="mode"
          data-command-mode
          :disabled="blocked"
          class="min-w-0 rounded-md border border-(--line-strong) bg-(--glass-control) px-2 py-1 text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
        >
          <option value="play">Play</option>
          <option value="next">Play next</option>
          <option value="queue">Add to queue</option>
        </select></label
      >
      <p
        v-if="error"
        role="alert"
        data-command-error
        class="break-words text-sm text-(--error-text)"
      >
        {{ error }}
      </p>
      <p v-if="blocked" role="status" class="text-sm text-(--muted-text)">
        {{ busy ? 'Running action…' : 'Another update is in progress. Please wait.' }}
      </p>
      <p v-if="!results.length" role="status" class="py-4 text-sm text-(--muted-text)">
        No results. Try a different search.
      </p>
      <div
        id="command-results"
        role="listbox"
        aria-label="Music and actions"
        :aria-busy="blocked"
        class="max-h-[min(22rem,45dvh)] min-h-0 overflow-y-auto"
      >
        <button
          v-for="(result, index) in results"
          :id="`command-option-${index}`"
          :key="result.id"
          type="button"
          role="option"
          :aria-selected="index === activeIndex"
          :aria-label="actionLabel(result)"
          :disabled="blocked"
          tabindex="-1"
          :data-command-result="result.id"
          class="flex w-full min-w-0 flex-col gap-0.5 rounded-md px-3 py-2 text-left text-sm hover:bg-(--surface-muted) focus-visible:ring-2 focus-visible:ring-(--focus-ring) aria-selected:bg-(--accent-soft) disabled:opacity-50"
          @pointermove="activeIndex = index"
          @click="run(result)"
        >
          <span class="w-full truncate">{{ actionLabel(result) }}</span
          ><span class="w-full truncate text-xs text-(--muted-text)"
            >{{ result.kind }} · {{ result.detail
            }}<template v-if="result.trackIds.length && result.kind !== 'selection'">
              · {{ result.trackIds.length }}
              {{ result.trackIds.length === 1 ? 'track' : 'tracks' }}</template
            ></span
          >
        </button>
      </div>
      <p class="text-xs text-(--muted-text)">
        ↑ ↓ to choose · Enter to run · Escape to close · Up to 40 results
      </p>
    </div>
  </LibraryDialog>
</template>
