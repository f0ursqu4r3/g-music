<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import type {
  MediaItem,
  Playlist,
  SmartPlaylistDefinition,
  SmartPlaylistField,
  SmartPlaylistPreview,
} from '@/api'
import {
  copyDefinition,
  describeRule,
  newRule,
  operatorsFor,
  operatorLabels,
  ruleFields,
  smartPresets,
  validateDefinition,
} from '@/lib/smart-playlists'
import LibraryDialog from './LibraryDialog.vue'
import { Button } from '@/components/ui/button'
const props = defineProps<{
  playlist?: Playlist
  tracks: MediaItem[]
  isUpdating?: boolean
  saveAction?: (playlist: Playlist) => Promise<unknown>
  previewAction?: (definition: SmartPlaylistDefinition) => Promise<SmartPlaylistPreview>
  freezeAction?: (id: string) => Promise<unknown>
  deleteAction?: (id: string) => Promise<unknown>
}>()
const emit = defineEmits<{ cancel: []; saved: [id: string]; deleted: [id: string] }>()
const id = props.playlist?.id ?? `playlist-${crypto.randomUUID()}`
const name = ref(props.playlist?.name ?? '')
const draft = ref(copyDefinition(props.playlist?.smart ?? smartPresets[0]!.definition))
const error = ref('')
const busy = ref(false)
const previewing = ref(false)
const preview = ref<SmartPlaylistPreview | null>(null)
const confirmation = ref<'freeze' | 'delete' | null>(null)
const title = props.playlist ? 'Edit smart playlist' : 'New smart playlist'
const blocked = computed(() => busy.value || !!props.isUpdating)
const trackNames = computed(() => new Map(props.tracks.map((track) => [track.id, track.title])))
let version = 0
let alive = true
watch(
  draft,
  () => {
    version++
    preview.value = null
  },
  { deep: true, flush: 'sync' },
)
onBeforeUnmount(() => {
  alive = false
  version++
})
function setField(index: number, event: Event): void {
  draft.value.rules[index] = newRule(
    (event.target as HTMLSelectElement).value as SmartPlaylistField,
  )
}
function definition(): SmartPlaylistDefinition {
  const result = copyDefinition(draft.value)
  result.rules = result.rules.map((rule) => ({
    ...rule,
    value: typeof rule.value === 'string' ? rule.value.trim() : rule.value,
  }))
  return result
}
function preset(index: number): void {
  const chosen = smartPresets[index]!
  draft.value = copyDefinition(chosen.definition)
  if (!name.value.trim()) name.value = chosen.name
  error.value = ''
}
async function showPreview(): Promise<void> {
  if (previewing.value || blocked.value) return
  const value = definition()
  error.value = validateDefinition(value)
  if (error.value) return
  const request = version
  previewing.value = true
  try {
    if (!props.previewAction) throw new Error('Preview is unavailable.')
    const result = await props.previewAction(value)
    if (alive && request === version) preview.value = result
  } catch (cause) {
    if (alive && request === version) error.value = message(cause)
  } finally {
    if (alive) previewing.value = false
  }
}
function message(cause: unknown): string {
  return `${cause instanceof Error ? cause.message : typeof cause === 'object' && cause && 'message' in cause ? cause.message : String(cause)} Your draft is kept. Try again.`
}
async function save(): Promise<void> {
  if (blocked.value || confirmation.value) return
  const value = definition()
  error.value = !name.value.trim() ? 'Enter a playlist name.' : validateDefinition(value)
  if (error.value) return
  await run(async () => {
    if (!props.saveAction) throw new Error('Playlist editing is unavailable.')
    await props.saveAction({ id, name: name.value.trim(), trackIds: [], smart: value })
    if (alive) emit('saved', id)
  })
}
async function confirm(): Promise<void> {
  if (blocked.value || !props.playlist) return
  await run(async () => {
    if (confirmation.value === 'freeze') {
      if (!props.freezeAction) throw new Error('Conversion is unavailable.')
      await props.freezeAction(id)
      if (alive) emit('saved', id)
    } else if (confirmation.value === 'delete') {
      if (!props.deleteAction) throw new Error('Playlist deletion is unavailable.')
      await props.deleteAction(id)
      if (alive) emit('deleted', id)
    }
  })
}
async function run(action: () => Promise<void>): Promise<void> {
  busy.value = true
  error.value = ''
  try {
    await action()
  } catch (cause) {
    if (alive) error.value = message(cause)
  } finally {
    if (alive) busy.value = false
  }
}
</script>
<template>
  <LibraryDialog
    :title="title"
    initial-focus="[data-smart-name]"
    :busy="busy"
    @close="emit('cancel')"
  >
    <form
      data-smart-form
      class="smart-editor flex min-w-0 shrink-0 flex-col gap-4"
      @submit.prevent="save"
    >
      <header class="flex items-center justify-between gap-3">
        <h2 class="text-lg font-semibold">{{ title }}</h2>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          :disabled="busy"
          aria-label="Close smart playlist editor"
          @click="emit('cancel')"
          >Close</Button
        >
      </header>
      <fieldset :disabled="blocked" class="flex min-w-0 flex-col gap-4">
        <label class="grid gap-1 text-sm"
          >Playlist name<input v-model="name" data-smart-name maxlength="200"
        /></label>
        <div class="flex flex-wrap gap-1.5" aria-label="Smart playlist presets">
          <Button
            v-for="(item, index) in smartPresets"
            :key="item.id"
            type="button"
            variant="outline"
            size="sm"
            :data-smart-preset="item.id"
            @click="preset(index)"
            >{{ item.name }}</Button
          >
        </div>
        <label class="flex items-center gap-2 text-sm"
          >Match<select v-model="draft.match" aria-label="Match rules">
            <option value="all">All rules</option>
            <option value="any">Any rule</option>
          </select></label
        >
        <ol class="flex flex-col gap-3">
          <li
            v-for="(rule, index) in draft.rules"
            :key="index"
            class="grid min-w-0 grid-cols-[1fr_1fr_auto] gap-2"
            data-smart-rule
          >
            <label class="grid min-w-0 gap-1 text-xs"
              >Rule {{ index + 1 }} field<select
                :value="rule.field"
                :aria-label="`Rule ${index + 1} field`"
                @change="setField(index, $event)"
              >
                <option v-for="(label, field) in ruleFields" :key="field" :value="field">
                  {{ label }}
                </option>
              </select></label
            >
            <label class="grid min-w-0 gap-1 text-xs"
              >Operator<select v-model="rule.operator" :aria-label="`Rule ${index + 1} operator`">
                <option
                  v-for="operator in operatorsFor(rule.field)"
                  :key="operator"
                  :value="operator"
                >
                  {{ operatorLabels[operator] }}
                </option>
              </select></label
            >
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class="self-end"
              :disabled="draft.rules.length === 1"
              :aria-label="`Remove rule ${index + 1}`"
              @click="draft.rules.splice(index, 1)"
              >Remove</Button
            >
            <label class="col-span-3 grid min-w-0 gap-1 text-xs"
              >{{
                rule.field === 'durationMs'
                  ? 'Value in milliseconds (180000 = 3 minutes)'
                  : rule.field === 'lastPlayedDays'
                    ? 'Days (never played matches not within)'
                    : 'Value'
              }}
              <select
                v-if="rule.field === 'favorite'"
                v-model="rule.value"
                :aria-label="`Rule ${index + 1} value`"
              >
                <option :value="true">Yes</option>
                <option :value="false">No</option>
              </select>
              <input
                v-else-if="['durationMs', 'playCount', 'lastPlayedDays'].includes(rule.field)"
                v-model.number="rule.value"
                type="number"
                step="1"
                :aria-label="`Rule ${index + 1} value`"
              />
              <input
                v-else
                v-model="rule.value"
                maxlength="200"
                :aria-label="`Rule ${index + 1} value`"
              />
            </label>
          </li>
        </ol>
        <Button
          type="button"
          variant="outline"
          size="sm"
          class="self-start"
          :disabled="draft.rules.length >= 20"
          @click="draft.rules.push(newRule())"
          >Add rule</Button
        >
        <div class="grid grid-cols-2 gap-2">
          <label class="grid min-w-0 gap-1 text-sm"
            >Sort by<select v-model="draft.sort.field" aria-label="Sort smart playlist">
              <option value="libraryOrder">Library order</option>
              <option value="title">Title</option>
              <option value="artist">Artist</option>
              <option value="album">Album</option>
              <option value="durationMs">Duration</option>
              <option value="playCount">Play count</option>
              <option value="lastPlayedAtMs">Last played</option>
            </select></label
          >
          <label class="grid min-w-0 gap-1 text-sm"
            >Direction<select v-model="draft.sort.direction" aria-label="Sort direction">
              <option value="asc">Ascending</option>
              <option value="desc">Descending</option>
            </select></label
          >
          <label class="col-span-2 grid gap-1 text-sm"
            >Track limit (optional)<input
              :value="draft.limit ?? ''"
              type="number"
              aria-label="Track limit"
              @input="
                draft.limit =
                  ($event.target as HTMLInputElement).value === ''
                    ? null
                    : Number(($event.target as HTMLInputElement).value)
              "
          /></label>
        </div>
      </fieldset>
      <p v-if="error" role="alert" data-smart-error class="text-sm text-(--error-text)">
        {{ error }}
      </p>
      <section v-if="preview" aria-label="Preview results" class="min-w-0 text-sm">
        <p role="status">
          {{ preview.totalMatches }} matches before limit · {{ preview.matches.length }} in
          playlist<span v-if="preview.matches.length > 20"> · showing first 20</span>
        </p>
        <ol class="mt-2 flex max-h-44 flex-col gap-2 overflow-y-auto">
          <li
            v-for="match in preview.matches.slice(0, 20)"
            :key="match.trackId"
            class="break-words"
            data-smart-preview-result
          >
            <strong>{{ trackNames.get(match.trackId) ?? match.trackId }}</strong>
            <p class="text-xs text-(--muted-text)">
              {{
                match.matchedRuleIndexes
                  .map((index) => (draft.rules[index] ? describeRule(draft.rules[index]!) : ''))
                  .filter(Boolean)
                  .join('; ')
              }}
            </p>
          </li>
        </ol>
      </section>
      <section
        v-if="confirmation"
        class="flex flex-col gap-3 rounded-md border border-(--line-strong) p-3"
        aria-label="Confirm playlist change"
      >
        <p class="text-sm">
          {{
            confirmation === 'freeze'
              ? 'Convert to a regular playlist? This keeps the saved name and current track order. Rules will stop updating. Unsaved edits will be discarded.'
              : 'Delete this playlist? Your tracks will stay in the library.'
          }}
        </p>
        <div class="flex flex-wrap justify-end gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            :disabled="busy"
            @click="confirmation = null"
            >Cancel</Button
          ><Button
            type="button"
            size="sm"
            data-smart-confirm
            :disabled="blocked"
            @click="confirm"
            >{{ confirmation === 'freeze' ? 'Convert playlist' : 'Delete playlist' }}</Button
          >
        </div>
      </section>
      <footer v-else class="flex flex-wrap gap-2">
        <template v-if="playlist"
          ><Button
            type="button"
            variant="ghost"
            size="sm"
            data-smart-freeze
            :disabled="blocked"
            @click="confirmation = 'freeze'"
            >Convert to regular playlist</Button
          ><Button
            type="button"
            variant="ghost"
            size="sm"
            data-smart-delete
            :disabled="blocked"
            @click="confirmation = 'delete'"
            >Delete playlist…</Button
          ></template
        >
        <div class="flex w-full justify-end gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            data-smart-preview
            :disabled="blocked || previewing"
            @click="showPreview"
            >{{ previewing ? 'Previewing…' : 'Preview' }}</Button
          ><Button type="submit" size="sm" :disabled="blocked">{{
            busy ? 'Saving…' : 'Save playlist'
          }}</Button>
        </div>
      </footer>
    </form>
  </LibraryDialog>
</template>
<style scoped>
.smart-editor input,
.smart-editor select {
  min-width: 0;
  width: 100%;
  border: 1px solid var(--line-strong);
  border-radius: 6px;
  background: var(--glass-control);
  color: var(--text);
  padding: 0.4rem 0.5rem;
  font-size: 0.875rem;
}
.smart-editor input:focus-visible,
.smart-editor select:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}
.smart-editor :disabled {
  opacity: 0.5;
}
</style>
