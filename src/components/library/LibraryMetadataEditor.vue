<script setup lang="ts">
import { computed, ref, watch } from "vue";

import type {
  EditableTrackMetadata,
  MediaItem,
  TrackMetadataUpdate,
} from "@/api";

export type MetadataEditKind = "album" | "artist" | "track";

export interface MetadataEditTarget {
  kind: MetadataEditKind;
  name: string;
  tracks: MediaItem[];
}

const props = defineProps<{
  target: MetadataEditTarget;
}>();

const emit = defineEmits<{
  cancel: [];
  save: [updates: TrackMetadataUpdate[]];
}>();

const title = ref("");
const artist = ref("");
const album = ref("");
const label = ref("");
const genres = ref("");

const isTrack = computed(() => props.target.kind === "track");
const isAlbum = computed(() => props.target.kind === "album");
const entityLabel = computed(() => {
  const labels: Record<MetadataEditKind, string> = {
    album: "Album",
    artist: "Artist",
    track: "Track",
  };
  return labels[props.target.kind];
});
const saveLabel = computed(() =>
  props.target.tracks.length === 1
    ? "Save changes"
    : `Save ${props.target.tracks.length} track changes`,
);

function resetForm(target: MetadataEditTarget): void {
  const track = target.tracks[0];
  title.value = track?.title ?? "";
  artist.value = track?.artist ?? "";
  album.value = track?.album ?? "";
  label.value = track?.label ?? "";
  genres.value = track?.genres?.join(", ") ?? "";
}

function optionalValue(value: string): string | null {
  const trimmed = value.trim();
  return trimmed || null;
}

function parseGenres(value: string): string[] {
  return value
    .split(",")
    .map((genre) => genre.trim())
    .filter(Boolean);
}

function save(): void {
  const updates = props.target.tracks.map<TrackMetadataUpdate>((track) => ({
    id: track.id,
    metadata: {
      album:
        isAlbum.value || isTrack.value
          ? optionalValue(album.value)
          : (track.album ?? null),
      artist: artist.value.trim(),
      genres: parseGenres(genres.value),
      label: optionalValue(label.value),
      title: isTrack.value ? title.value.trim() : track.title,
    } satisfies EditableTrackMetadata,
  }));
  emit("save", updates);
}

watch(() => props.target, resetForm, { immediate: true });
</script>

<template>
  <section
    class="absolute inset-0 z-50 grid place-items-center bg-black/60 p-5 backdrop-blur-sm"
    role="dialog"
    aria-labelledby="metadata-editor-title"
    aria-modal="true"
    @click.self="emit('cancel')"
    @keydown.esc="emit('cancel')"
  >
    <form
      class="w-full max-w-112 rounded-2xl border border-(--line-strong) bg-[oklch(0.11_0.014_260/0.98)] p-6 shadow-2xl"
      @submit.prevent="save"
    >
      <header class="flex items-start justify-between gap-4">
        <div>
          <p
            class="text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
          >
            Metadata
          </p>
          <h2
            id="metadata-editor-title"
            class="mt-1 text-xl font-semibold text-(--text)"
          >
            Edit {{ entityLabel }}
          </h2>
          <p class="mt-1 text-sm text-(--muted-text)">
            {{ props.target.name }}
          </p>
        </div>
        <button
          class="rounded-md px-2 py-1 text-sm text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
          type="button"
          aria-label="Close metadata editor"
          @click="emit('cancel')"
        >
          Esc
        </button>
      </header>

      <p
        v-if="props.target.tracks.length > 1"
        class="mt-5 rounded-lg border border-(--line) bg-(--surface-muted) px-3 py-2 text-xs text-(--muted-text)"
      >
        These changes apply to {{ props.target.tracks.length }} tracks.
      </p>

      <div class="mt-5 grid gap-4">
        <label v-if="isTrack" class="grid gap-1.5 text-sm text-(--text)">
          Track title
          <input
            v-model="title"
            class="rounded-md border border-(--line-strong) bg-transparent px-3 py-2 text-sm outline-none placeholder:text-(--subtle-text) focus:border-(--focus-ring) focus:ring-2 focus:ring-(--focus-ring)/30"
            data-metadata-field="title"
            required
          />
        </label>
        <label class="grid gap-1.5 text-sm text-(--text)">
          Artist
          <input
            v-model="artist"
            class="rounded-md border border-(--line-strong) bg-transparent px-3 py-2 text-sm outline-none placeholder:text-(--subtle-text) focus:border-(--focus-ring) focus:ring-2 focus:ring-(--focus-ring)/30"
            data-metadata-field="artist"
            required
          />
        </label>
        <label
          v-if="isAlbum || isTrack"
          class="grid gap-1.5 text-sm text-(--text)"
        >
          Album
          <input
            v-model="album"
            class="rounded-md border border-(--line-strong) bg-transparent px-3 py-2 text-sm outline-none placeholder:text-(--subtle-text) focus:border-(--focus-ring) focus:ring-2 focus:ring-(--focus-ring)/30"
            data-metadata-field="album"
          />
        </label>
        <label class="grid gap-1.5 text-sm text-(--text)">
          Label
          <input
            v-model="label"
            class="rounded-md border border-(--line-strong) bg-transparent px-3 py-2 text-sm outline-none placeholder:text-(--subtle-text) focus:border-(--focus-ring) focus:ring-2 focus:ring-(--focus-ring)/30"
            data-metadata-field="label"
          />
        </label>
        <label class="grid gap-1.5 text-sm text-(--text)">
          Genres
          <input
            v-model="genres"
            class="rounded-md border border-(--line-strong) bg-transparent px-3 py-2 text-sm outline-none placeholder:text-(--subtle-text) focus:border-(--focus-ring) focus:ring-2 focus:ring-(--focus-ring)/30"
            data-metadata-field="genres"
            placeholder="Ambient, Electronic"
          />
        </label>
      </div>

      <footer class="mt-6 flex justify-end gap-3">
        <button
          class="rounded-md px-3 py-2 text-sm font-medium text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
          type="button"
          @click="emit('cancel')"
        >
          Cancel
        </button>
        <button
          class="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-(--accent-ink) hover:brightness-110 focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
          data-metadata-editor-save
          type="submit"
        >
          {{ saveLabel }}
        </button>
      </footer>
    </form>
  </section>
</template>
