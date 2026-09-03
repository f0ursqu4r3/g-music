<script setup lang="ts">
import { computed, ref, watch } from "vue";

import type { MediaItem, Playlist } from "@/api";

const props = defineProps<{
  playlist?: Playlist;
  tracks: MediaItem[];
}>();

const emit = defineEmits<{
  cancel: [];
  delete: [id: string];
  save: [playlist: Playlist];
}>();

const name = ref("");
const selectedTrackIds = ref<Set<string>>(new Set());
const deleteConfirmationOpen = ref(false);

const isEditing = computed(() => Boolean(props.playlist));
const title = computed(() =>
  isEditing.value ? "Edit playlist" : "New playlist",
);

function resetForm(playlist: Playlist | undefined): void {
  name.value = playlist?.name ?? "";
  selectedTrackIds.value = new Set(playlist?.trackIds ?? []);
  deleteConfirmationOpen.value = false;
}

function toggleTrack(id: string): void {
  const nextTrackIds = new Set(selectedTrackIds.value);
  if (nextTrackIds.has(id)) {
    nextTrackIds.delete(id);
  } else {
    nextTrackIds.add(id);
  }
  selectedTrackIds.value = nextTrackIds;
}

function save(): void {
  const trimmedName = name.value.trim();
  if (!trimmedName) {
    return;
  }

  emit("save", {
    id: props.playlist?.id ?? `playlist-${crypto.randomUUID()}`,
    name: trimmedName,
    trackIds: props.tracks
      .filter((track) => selectedTrackIds.value.has(track.id))
      .map((track) => track.id),
  });
}

function deletePlaylist(): void {
  if (!props.playlist) {
    return;
  }

  if (deleteConfirmationOpen.value) {
    emit("delete", props.playlist.id);
    return;
  }

  deleteConfirmationOpen.value = true;
}

watch(() => props.playlist, resetForm, { immediate: true });
</script>

<template>
  <section
    aria-labelledby="playlist-editor-title"
    aria-modal="true"
    class="absolute inset-0 z-50 grid place-items-center bg-black/60 p-5 backdrop-blur-sm"
    role="dialog"
    @click.self="emit('cancel')"
    @keydown.esc="emit('cancel')"
  >
    <form
      class="w-full max-w-130 rounded-2xl border border-(--line-strong) bg-[oklch(0.11_0.014_260/0.98)] p-6 shadow-2xl"
      @submit.prevent="save"
    >
      <header class="flex items-start justify-between gap-4">
        <div>
          <p
            class="text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
          >
            Playlist
          </p>
          <h2
            id="playlist-editor-title"
            class="mt-1 text-xl font-semibold text-(--text)"
          >
            {{ title }}
          </h2>
        </div>
        <button
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
              type="checkbox"
              @change="toggleTrack(track.id)"
            />
            <span class="min-w-0 truncate">{{ track.title }}</span>
            <span class="ml-auto shrink-0 text-xs text-(--muted-text)">
              {{ track.artist }}
            </span>
          </label>
        </div>
      </fieldset>

      <footer class="mt-6 flex items-center justify-between gap-3">
        <button
          v-if="props.playlist"
          class="rounded-md px-3 py-2 text-sm font-medium text-red-300 hover:bg-red-500/15 focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
          data-playlist-editor-delete
          type="button"
          @click="deletePlaylist"
        >
          {{ deleteConfirmationOpen ? "Confirm delete" : "Delete playlist" }}
        </button>
        <span v-else />
        <div class="flex gap-3">
          <button
            class="rounded-md px-3 py-2 text-sm font-medium text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
            type="button"
            @click="emit('cancel')"
          >
            Cancel
          </button>
          <button
            class="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-(--accent-ink) hover:brightness-110 focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
            data-playlist-editor-save
            type="submit"
          >
            Save playlist
          </button>
        </div>
      </footer>
    </form>
  </section>
</template>
