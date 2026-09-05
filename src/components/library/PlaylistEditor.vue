<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import type { MediaItem, Playlist } from '@/api'
import LibraryDialog from './LibraryDialog.vue'

const props = defineProps<{
  playlist?: Playlist
  tracks: MediaItem[]
  saveAction?: (playlist: Playlist) => Promise<unknown>
  deleteAction?: (id: string) => Promise<unknown>
}>()

const emit = defineEmits<{
  cancel: []
  delete: [id: string]
  save: [playlist: Playlist]
  saved: []
}>()

const name = ref('')
const selectedTrackIds = ref<Set<string>>(new Set())
const deleteConfirmationOpen = ref(false)
const busy = ref(false)
const error = ref('')

const isEditing = computed(() => Boolean(props.playlist))
const title = computed(() => (isEditing.value ? 'Edit playlist' : 'New playlist'))

function resetForm(playlist: Playlist | undefined): void {
  name.value = playlist?.name ?? ''
  selectedTrackIds.value = new Set(playlist?.trackIds ?? [])
  deleteConfirmationOpen.value = false
}

function toggleTrack(id: string): void {
  const nextTrackIds = new Set(selectedTrackIds.value)
  if (nextTrackIds.has(id)) {
    nextTrackIds.delete(id)
  } else {
    nextTrackIds.add(id)
  }
  selectedTrackIds.value = nextTrackIds
}

async function save(): Promise<void> {
  if (busy.value) return
  const trimmedName = name.value.trim()
  if (!trimmedName) {
    return
  }

  const originalIds = props.playlist?.trackIds ?? []
  const originalSet = new Set(originalIds)
  const playlist = {
    id: props.playlist?.id ?? `playlist-${crypto.randomUUID()}`,
    name: trimmedName,
    trackIds: [
      ...originalIds.filter((id) => selectedTrackIds.value.has(id)),
      ...props.tracks
        .filter((track) => selectedTrackIds.value.has(track.id) && !originalSet.has(track.id))
        .map((track) => track.id),
    ],
  }
  busy.value = true
  error.value = ''
  try {
    if (props.saveAction) {
      await props.saveAction(playlist)
      emit('saved')
    } else emit('save', playlist)
  } catch (cause) {
    error.value = `Could not save playlist. ${cause instanceof Error ? cause.message : String(cause)} Your draft is kept. Try Save again.`
  } finally {
    busy.value = false
  }
}

async function deletePlaylist(): Promise<void> {
  if (busy.value) return
  if (!props.playlist) {
    return
  }

  if (deleteConfirmationOpen.value) {
    busy.value = true
    error.value = ''
    try {
      if (props.deleteAction) {
        await props.deleteAction(props.playlist.id)
        emit('saved')
      } else emit('delete', props.playlist.id)
    } catch (cause) {
      error.value = `Could not delete playlist. ${cause instanceof Error ? cause.message : String(cause)} Try again.`
    } finally {
      busy.value = false
    }
    return
  }

  deleteConfirmationOpen.value = true
}

watch(() => props.playlist, resetForm, { immediate: true })
</script>

<template>
  <LibraryDialog :title="title" :busy="busy" @close="emit('cancel')">
    <form class="w-full min-w-0" @submit.prevent="save">
      <header class="flex items-start justify-between gap-4">
        <div>
          <p class="text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase">
            Playlist
          </p>
          <h2 id="playlist-editor-title" class="mt-1 text-xl font-semibold text-(--text)">
            {{ title }}
          </h2>
        </div>
        <button
          :disabled="busy"
          aria-label="Close playlist editor"
          class="rounded-md px-2 py-1 text-sm text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
          type="button"
          @click="emit('cancel')"
        >
          Esc
        </button>
      </header>

      <label class="mt-5 grid gap-1.5 text-sm text-(--text)">
        Playlist name
        <input
          v-model="name"
          :disabled="busy"
          class="rounded-md border border-(--line-strong) bg-transparent px-3 py-2 text-sm outline-none placeholder:text-(--subtle-text) focus:border-(--focus-ring) focus:ring-2 focus:ring-(--focus-ring)/30"
          data-playlist-field="name"
          required
        />
      </label>

      <fieldset class="mt-5 grid gap-2">
        <legend class="text-sm font-medium text-(--text)">Tracks</legend>
        <div
          class="max-h-72 overflow-y-auto rounded-md border border-(--line) bg-(--surface-muted) p-2"
        >
          <label
            v-for="track in props.tracks"
            :key="track.id"
            :data-playlist-track="track.id"
            class="flex cursor-pointer items-center gap-3 rounded px-2 py-1.5 text-sm text-(--text) hover:bg-(--line)"
          >
            <input
              :checked="selectedTrackIds.has(track.id)"
              :disabled="busy"
              type="checkbox"
              @change="toggleTrack(track.id)"
            />
            <span class="min-w-0 truncate">{{ track.title }}</span>
            <span class="ml-auto max-w-1/2 truncate text-xs text-(--muted-text)">
              {{ track.artist }}
            </span>
          </label>
        </div>
      </fieldset>

      <p v-if="error" role="alert" class="mt-3 break-words text-sm text-(--error-text)">
        {{ error }}
      </p>
      <footer
        class="sticky bottom-0 mt-6 flex flex-wrap items-center justify-between gap-3 bg-(--dialog-surface,var(--surface)) py-2"
      >
        <button
          v-if="props.playlist"
          :disabled="busy"
          class="rounded-md px-3 py-2 text-sm font-medium text-red-300 hover:bg-red-500/15 focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
          data-playlist-editor-delete
          type="button"
          @click="deletePlaylist"
        >
          {{ deleteConfirmationOpen ? 'Confirm delete' : 'Delete playlist' }}
        </button>
        <span v-else />
        <div class="flex gap-3">
          <button
            :disabled="busy"
            class="rounded-md px-3 py-2 text-sm font-medium text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
            type="button"
            @click="emit('cancel')"
          >
            Cancel
          </button>
          <button
            class="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-(--accent-ink) hover:brightness-110 focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
            data-playlist-editor-save
            :disabled="busy || !name.trim()"
            type="submit"
          >
            Save playlist
          </button>
        </div>
      </footer>
    </form>
  </LibraryDialog>
</template>
