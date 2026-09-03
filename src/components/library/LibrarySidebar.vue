<script setup lang="ts">
import {
  Clock3,
  Disc3,
  Heart,
  ListMusic,
  Mic2,
  Pencil,
  Plus,
} from "lucide-vue-next";
import { nextTick, ref, watch } from "vue";

import { ScrollArea } from "@/components/ui/scroll-area";
import type { Playlist } from "@/api";
import type { LibraryCollection } from "./types";

const props = defineProps<{
  activeCollection: LibraryCollection;
  activePlaylistId?: string;
  isCreatingPlaylist?: boolean;
  playlists: Playlist[];
}>();

const emit = defineEmits<{
  selectCollection: [collection: LibraryCollection];
  selectPlaylist: [id: string];
  createPlaylist: [name: string];
  cancelPlaylistCreation: [];
  newPlaylist: [];
  editPlaylist: [playlist: Playlist];
  openImport: [];
}>();

const newPlaylistName = ref("");
const newPlaylistInput = ref<HTMLInputElement>();

function saveNewPlaylist(): void {
  const name = newPlaylistName.value.trim();
  if (name) {
    emit("createPlaylist", name);
  }
}

function cancelNewPlaylist(): void {
  emit("cancelPlaylistCreation");
}

function playlistIcon(id: string) {
  return id === "favorites" ? Heart : id === "most-played" ? Clock3 : ListMusic;
}

watch(
  () => props.isCreatingPlaylist,
  async (isCreating) => {
    newPlaylistName.value = "";
    if (isCreating) {
      await nextTick();
      newPlaylistInput.value?.focus();
    }
  },
);
</script>

<template>
  <aside
    class="col-start-1 row-start-1 mt-8 min-h-0 max-[760px]:hidden"
    data-library-sidebar
  >
    <ScrollArea class="size-full">
      <div class="p-4">
        <nav class="grid gap-0.5" aria-label="Library navigation">
          <div
            class="mb-1 flex items-center justify-between px-2.5"
            data-library-heading="library"
          >
            <p
              class="text-[0.61rem] font-semibold tracking-[0.06em] text-(--subtle-text)"
            >
              Library
            </p>
            <button
              aria-label="Import music"
              class="grid size-6 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4"
              type="button"
              @click="emit('openImport')"
            >
              <Plus aria-hidden="true" />
            </button>
          </div>
          <button
            v-for="collection in [
              ['tracks', ListMusic, 'Tracks'],
              ['albums', Disc3, 'Albums'],
              ['artists', Mic2, 'Artists'],
            ] as const"
            :key="collection[0]"
            :aria-current="
              props.activeCollection === collection[0] ? 'page' : undefined
            "
            class="flex min-h-8.5 w-full cursor-pointer items-center gap-2.5 rounded-md border-0 bg-transparent px-2.5 text-left text-[0.82rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.7_0.03_262/0.15)] aria-[current=page]:text-(--text) aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
            :data-collection="collection[0]"
            type="button"
            @click="emit('selectCollection', collection[0])"
          >
            <component :is="collection[1]" aria-hidden="true" />{{
              collection[2]
            }}
          </button>
          <span
            class="flex min-h-8.5 items-center gap-2.5 rounded-md px-2.5 text-[0.82rem] text-(--muted-text) [&>svg]:size-4"
            data-library-destination="playlists"
          >
            <ListMusic aria-hidden="true" />Playlists
          </span>
        </nav>

        <nav
          class="mt-5 grid gap-0.5"
          aria-label="Playlists"
          data-library-playlists
        >
          <div class="mb-1 flex items-center justify-between px-2.5">
            <p
              class="text-[0.61rem] font-semibold tracking-[0.06em] text-(--subtle-text)"
            >
              Playlists
            </p>
            <button
              aria-label="New playlist"
              :disabled="props.isCreatingPlaylist"
              class="grid size-6 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) disabled:cursor-default disabled:opacity-40 [&>svg]:size-4"
              type="button"
              @click="emit('newPlaylist')"
            >
              <Plus aria-hidden="true" />
            </button>
          </div>
          <form
            v-if="props.isCreatingPlaylist"
            class="flex min-h-8 items-center gap-2 rounded-md bg-[oklch(0.7_0.03_262/0.15)] px-2.5"
            data-new-playlist-editor
            @submit.prevent="saveNewPlaylist"
          >
            <ListMusic aria-hidden="true" class="size-4 shrink-0 text-accent" />
            <input
              ref="newPlaylistInput"
              v-model="newPlaylistName"
              aria-label="New playlist name"
              class="min-w-0 flex-1 bg-transparent text-[0.79rem] text-(--text) outline-none placeholder:text-(--subtle-text)"
              placeholder="New playlist"
              @keydown.esc.prevent="cancelNewPlaylist"
            />
          </form>
          <div
            v-for="playlist in props.playlists"
            :key="playlist.id"
            class="group flex min-h-8 items-center gap-1 rounded-md text-[0.79rem] text-(--muted-text)"
          >
            <button
              :aria-current="
                props.activePlaylistId === playlist.id ? 'page' : undefined
              "
              class="flex min-h-8 min-w-0 flex-1 cursor-pointer items-center gap-2.5 rounded-md border-0 bg-transparent px-2.5 text-left transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.7_0.03_262/0.15)] aria-[current=page]:text-(--text) [&>svg]:size-4"
              :data-playlist-id="playlist.id"
              type="button"
              @click="emit('selectPlaylist', playlist.id)"
            >
              <component :is="playlistIcon(playlist.id)" aria-hidden="true" />
              <span class="truncate">{{ playlist.name }}</span>
            </button>
            <button
              v-if="
                playlist.id !== 'favorites' && playlist.id !== 'most-played'
              "
              :aria-label="`Edit ${playlist.name}`"
              class="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) opacity-0 transition-opacity hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) group-hover:opacity-100 focus-visible:opacity-100 [&>svg]:size-3.5"
              type="button"
              @click="emit('editPlaylist', playlist)"
            >
              <Pencil aria-hidden="true" />
            </button>
          </div>
        </nav>
      </div>
    </ScrollArea>
  </aside>
</template>

<style scoped>
/* The sidebar has no component-specific rules beyond utility classes. */
</style>
