<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import type {
  EditableTrackMetadata,
  MediaItem,
  TrackMetadataUpdate,
} from "@/api";
import LibraryDialog from "./LibraryDialog.vue";
export type MetadataEditKind = "album" | "artist" | "track";
export interface MetadataEditTarget {
  kind: MetadataEditKind;
  name: string;
  tracks: MediaItem[];
}
const props = defineProps<{
  target: MetadataEditTarget;
  saveAction?: (updates: TrackMetadataUpdate[]) => Promise<unknown>;
  resetAction?: (ids: string[]) => Promise<unknown>;
}>();
const emit = defineEmits<{
  cancel: [];
  save: [updates: TrackMetadataUpdate[]];
  saved: [];
}>();
type Field = keyof EditableTrackMetadata;
const fields: { key: Field; label: string; optional?: boolean }[] = [
  { key: "title", label: "Track title" },
  { key: "artist", label: "Artist" },
  { key: "album", label: "Album", optional: true },
  { key: "label", label: "Label", optional: true },
  { key: "genres", label: "Genres", optional: true },
];
const visibleFields = computed(() =>
  fields.filter(
    (field) =>
      (field.key !== "title" || props.target.kind === "track") &&
      (field.key !== "album" || props.target.kind !== "artist"),
  ),
);
const values = reactive<Record<Field, string>>({
  title: "",
  artist: "",
  album: "",
  label: "",
  genres: "",
});
const mixed = reactive(new Set<Field>());
const dirty = reactive(new Set<Field>());
const busy = ref(false);
const error = ref("");
const confirmingReset = ref(false);
const entityLabel = computed(
  () => props.target.kind[0]!.toUpperCase() + props.target.kind.slice(1),
);
function stringValue(track: MediaItem, key: Field): string {
  return key === "genres"
    ? (track.genres ?? []).join(", ")
    : (track[key] ?? "");
}
watch(
  () => props.target,
  (target) => {
    mixed.clear();
    dirty.clear();
    error.value = "";
    confirmingReset.value = false;
    for (const { key } of fields) {
      const first = target.tracks[0] ? stringValue(target.tracks[0], key) : "";
      if (target.tracks.some((track) => stringValue(track, key) !== first))
        mixed.add(key);
      values[key] = mixed.has(key) ? "" : first;
    }
  },
  { immediate: true },
);
function edit(key: Field, value: string): void {
  values[key] = value;
  dirty.add(key);
  error.value = "";
}
function metadata(): Partial<EditableTrackMetadata> {
  const result: Partial<EditableTrackMetadata> = {};
  for (const { key } of visibleFields.value) {
    if (!dirty.has(key)) continue;
    const value = values[key].trim();
    if (key === "genres")
      result.genres = [
        ...new Set(
          value
            .split(",")
            .map((item) => item.trim())
            .filter(Boolean),
        ),
      ];
    else if (key === "title" || key === "artist") result[key] = value;
    else result[key] = value || null;
  }
  return result;
}
async function save(): Promise<void> {
  if (busy.value || !dirty.size || confirmingReset.value) return;
  for (const field of visibleFields.value) {
    if (!field.optional && dirty.has(field.key) && !values[field.key].trim()) {
      error.value = `${field.label} is required. Enter a value or cancel the edit.`;
      return;
    }
  }
  const updates = props.target.tracks.map((track) => ({
    id: track.id,
    metadata: metadata(),
  }));
  busy.value = true;
  error.value = "";
  try {
    if (props.saveAction) {
      await props.saveAction(updates);
      emit("saved");
    } else emit("save", updates);
  } catch (cause) {
    error.value = `Could not save metadata. ${cause instanceof Error ? cause.message : String(cause)} Your draft is kept. Try Save again.`;
  } finally {
    busy.value = false;
  }
}
async function reset(): Promise<void> {
  if (!props.resetAction || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await props.resetAction(props.target.tracks.map((track) => track.id));
    emit("saved");
  } catch (cause) {
    error.value = `Could not reset metadata. ${cause instanceof Error ? cause.message : String(cause)} Your draft is kept.`;
  } finally {
    busy.value = false;
    confirmingReset.value = false;
  }
}
</script>
<template>
  <LibraryDialog
    :title="`Edit ${entityLabel}`"
    :busy="busy"
    @close="emit('cancel')"
  >
    <form novalidate @submit.prevent="save">
      <header class="flex items-start justify-between gap-4">
        <div class="min-w-0">
          <h2 class="text-xl font-semibold">Edit {{ entityLabel }}</h2>
          <p class="mt-1 break-words text-sm text-(--muted-text)">
            {{ target.name }}
          </p>
        </div>
        <button
          type="button"
          aria-label="Close metadata editor"
          :disabled="busy"
          @click="emit('cancel')"
        >
          Esc
        </button>
      </header>
      <p
        v-if="target.tracks.length > 1"
        class="mt-3 text-xs text-(--muted-text)"
      >
        Only changed fields apply to {{ target.tracks.length }} tracks. Mixed
        values stay unchanged unless edited or cleared.
      </p>
      <div class="mt-4 grid gap-3">
        <label
          v-for="field in visibleFields"
          :key="field.key"
          class="grid gap-1 text-sm"
        >
          <span class="flex justify-between"
            >{{ field.label
            }}<button
              v-if="field.optional"
              :data-metadata-clear="field.key"
              :disabled="busy"
              type="button"
              class="text-xs text-(--muted-text) underline"
              @click="edit(field.key, '')"
            >
              Clear {{ field.label.toLowerCase() }}
            </button></span
          >
          <input
            :value="values[field.key]"
            :data-metadata-field="field.key"
            :disabled="busy"
            :placeholder="
              mixed.has(field.key) && !dirty.has(field.key)
                ? 'Mixed values'
                : field.key === 'genres'
                  ? 'Ambient, Electronic'
                  : ''
            "
            class="rounded-md border border-(--line-strong) bg-transparent px-3 py-2 outline-none focus:ring-2 focus:ring-(--focus-ring)"
            @input="edit(field.key, ($event.target as HTMLInputElement).value)"
          />
        </label>
      </div>
      <p
        v-if="error"
        role="alert"
        class="mt-3 break-words text-sm text-(--error-text)"
      >
        {{ error }}
      </p>
      <div
        v-if="confirmingReset"
        class="mt-4 rounded-md border border-(--line-strong) p-3 text-sm"
      >
        <p>
          Restore stored provider metadata for
          {{ target.tracks.length }} tracks?
          {{
            dirty.size
              ? "This discards your unsaved draft."
              : "This removes your saved metadata edits."
          }}
        </p>
        <div class="mt-3 flex flex-wrap gap-3">
          <button
            data-cancel-metadata-reset
            type="button"
            :disabled="busy"
            @click="confirmingReset = false"
          >
            Keep editing</button
          ><button
            data-confirm-metadata-reset
            type="button"
            :disabled="busy"
            @click="reset"
          >
            {{ dirty.size ? "Discard draft and reset" : "Confirm reset" }}
          </button>
        </div>
      </div>
      <footer
        class="sticky bottom-0 mt-4 flex flex-wrap items-center justify-end gap-3 bg-(--dialog-surface,var(--surface)) py-2"
      >
        <button
          v-if="resetAction"
          data-metadata-reset
          type="button"
          :disabled="busy"
          class="mr-auto text-xs underline"
          @click="confirmingReset = true"
        >
          Reset to provider metadata
        </button>
        <button
          type="button"
          :disabled="busy"
          class="rounded-md px-3 py-2 text-sm"
          @click="emit('cancel')"
        >
          Cancel
        </button>
        <button
          data-metadata-editor-save
          type="submit"
          :disabled="busy || !dirty.size || confirmingReset"
          class="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-(--accent-ink) disabled:opacity-50"
        >
          {{
            busy
              ? "Saving…"
              : target.tracks.length > 1
                ? `Save ${target.tracks.length} track changes`
                : "Save changes"
          }}
        </button>
      </footer>
    </form>
  </LibraryDialog>
</template>
