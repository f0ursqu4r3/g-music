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
import { ReorderGroup, ReorderItem } from "motion-v";
import { computed, nextTick, ref, watch } from "vue";

import { ScrollArea } from "@/components/ui/scroll-area";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import type { Playlist } from "@/api";
import type { LibraryCollection } from "./types";

const props = defineProps<{
  activeCollection: LibraryCollection;
  activePlaylistId?: string;
  isCreatingPlaylist?: boolean;
  isUpdating?: boolean;
  playlists: Playlist[];
}>();

const emit = defineEmits<{
  selectCollection: [collection: LibraryCollection];
  selectPlaylist: [id: string];
  createPlaylist: [name: string];
  cancelPlaylistCreation: [];
  newPlaylist: [];
  deletePlaylist: [playlist: Playlist];
  editPlaylist: [playlist: Playlist];
  openImport: [];
  playPlaylist: [playlist: Playlist];
  reorderPlaylists: [playlistIds: string[]];
}>();

const newPlaylistName = ref("");
const newPlaylistInput = ref<HTMLInputElement>();
const activeReorderId = ref<string>();
const isReorderPending = ref(false);
const userPlaylists = computed(() =>
  props.playlists.filter((playlist) => !isDefaultPlaylist(playlist)),
);
const defaultPlaylists = computed(() =>
  props.playlists.filter((playlist) => isDefaultPlaylist(playlist)),
);
const reorderablePlaylists = ref<Playlist[]>([...userPlaylists.value]);
const playlistReorderTransition = {
  damping: 42,
  stiffness: 650,
  type: "spring" as const,
};

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

function isDefaultPlaylist(playlist: Playlist): boolean {
  return playlist.id === "favorites" || playlist.id === "most-played";
}

function startPlaylistReorder(id: string): void {
  activeReorderId.value = id;
}

function previewPlaylistReorder(playlists: Playlist[]): void {
  reorderablePlaylists.value = playlists;
}

function finishPlaylistReorder(): void {
  const id = activeReorderId.value;
  activeReorderId.value = undefined;
  if (!id || props.isUpdating) {
    reorderablePlaylists.value = [...userPlaylists.value];
    return;
  }

  const playlistIds = reorderablePlaylists.value.map((playlist) => playlist.id);
  if (
    playlistIds.every(
      (playlistId, index) => playlistId === userPlaylists.value[index]?.id,
    )
  ) {
    reorderablePlaylists.value = [...userPlaylists.value];
    return;
  }

  isReorderPending.value = true;
  emit("reorderPlaylists", playlistIds);
}

function movePlaylist(id: string, direction: -1 | 1): void {
  const from = reorderablePlaylists.value.findIndex(
    (playlist) => playlist.id === id,
  );
  const to = from + direction;
  if (
    props.isUpdating ||
    from === -1 ||
    to < 0 ||
    to >= reorderablePlaylists.value.length
  ) {
    return;
  }

  const playlists = [...reorderablePlaylists.value];
  const [playlist] = playlists.splice(from, 1);
  playlists.splice(to, 0, playlist!);
  reorderablePlaylists.value = playlists;
  isReorderPending.value = true;
  emit(
    "reorderPlaylists",
    playlists.map((playlist) => playlist.id),
  );
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

watch(userPlaylists, (playlists) => {
  reorderablePlaylists.value = [...playlists];
  activeReorderId.value = undefined;
  isReorderPending.value = false;
});

watch(
  () => props.isUpdating,
  (isUpdating) => {
    if (!isUpdating && isReorderPending.value) {
      reorderablePlaylists.value = [...userPlaylists.value];
      isReorderPending.value = false;
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
              props.activePlaylistId === undefined &&
              props.activeCollection === collection[0]
                ? 'page'
                : undefined
            "
            class="-mx-4 flex min-h-8.5 w-[calc(100%+2rem)] cursor-pointer items-center gap-2.5 rounded-none border-0 bg-transparent px-6.5 text-left text-[0.82rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.7_0.03_262/0.15)] aria-[current=page]:text-(--text) aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
            :data-collection="collection[0]"
            type="button"
            @click="emit('selectCollection', collection[0])"
          >
            <component :is="collection[1]" aria-hidden="true" />
            {{ collection[2] }}
          </button>
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
          <ContextMenu
            v-for="playlist in defaultPlaylists"
            :key="playlist.id"
            @update:open="
              (isOpen) => isOpen && emit('selectPlaylist', playlist.id)
            "
          >
            <ContextMenuTrigger as-child>
              <div
                :data-current="props.activePlaylistId === playlist.id"
                :data-default-playlist-row="playlist.id"
                class="group -mx-4 flex min-h-8 w-[calc(100%+2rem)] items-center gap-1 rounded-none text-[0.79rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) data-[current=true]:bg-[oklch(0.7_0.03_262/0.15)] data-[current=true]:text-(--text)"
              >
                <button
                  :aria-current="
                    props.activePlaylistId === playlist.id ? 'page' : undefined
                  "
                  class="flex min-h-8 min-w-0 flex-1 cursor-pointer items-center gap-2.5 rounded-none border-0 bg-transparent px-6.5 text-left aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
                  :data-playlist-id="playlist.id"
                  type="button"
                  @click="emit('selectPlaylist', playlist.id)"
                >
                  <component
                    :is="playlistIcon(playlist.id)"
                    aria-hidden="true"
                  />
                  <span class="truncate">{{ playlist.name }}</span>
                </button>
              </div>
            </ContextMenuTrigger>
            <ContextMenuContent data-playlist-context-menu>
              <ContextMenuItem @select="emit('playPlaylist', playlist)">
                Play playlist
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenu>
          <ReorderGroup
            as="div"
            axis="y"
            class="grid gap-0.5"
            data-playlist-reorder-list
            :values="reorderablePlaylists"
            @update:values="previewPlaylistReorder"
          >
            <ContextMenu
              v-for="playlist in reorderablePlaylists"
              :key="playlist.id"
              @update:open="
                (isOpen) => isOpen && emit('selectPlaylist', playlist.id)
              "
            >
              <ContextMenuTrigger as-child>
                <ReorderItem
                  as="div"
                  :aria-label="`Drag ${playlist.name} to reorder`"
                  :drag="props.isUpdating ? false : 'y'"
                  :drag-momentum="false"
                  :on-drag-end="finishPlaylistReorder"
                  :on-drag-start="() => startPlaylistReorder(playlist.id)"
                  :transition="playlistReorderTransition"
                  :value="playlist"
                  :data-current="props.activePlaylistId === playlist.id"
                  :data-playlist-reorder-item="playlist.id"
                  class="group -mx-4 flex min-h-8 w-[calc(100%+2rem)] cursor-grab items-center gap-1 rounded-none pr-4 text-[0.79rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) active:cursor-grabbing data-[current=true]:bg-[oklch(0.7_0.03_262/0.15)] data-[current=true]:text-(--text)"
                >
                  <button
                    :aria-current="
                      props.activePlaylistId === playlist.id
                        ? 'page'
                        : undefined
                    "
                    class="flex min-h-8 min-w-0 flex-1 cursor-pointer items-center gap-2.5 rounded-none border-0 bg-transparent px-6.5 text-left aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
                    :data-playlist-id="playlist.id"
                    type="button"
                    @click="emit('selectPlaylist', playlist.id)"
                  >
                    <component
                      :is="playlistIcon(playlist.id)"
                      aria-hidden="true"
                    />
                    <span class="truncate">{{ playlist.name }}</span>
                  </button>
                  <button
                    :aria-label="`Move ${playlist.name} up`"
                    class="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) opacity-0 transition-opacity hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) group-hover:opacity-100 focus-visible:opacity-100"
                    :disabled="
                      props.isUpdating ||
                      reorderablePlaylists.findIndex(
                        (candidate) => candidate.id === playlist.id,
                      ) === 0
                    "
                    type="button"
                    @click.stop="movePlaylist(playlist.id, -1)"
                  >
                    <span aria-hidden="true">↑</span>
                  </button>
                  <button
                    :aria-label="`Move ${playlist.name} down`"
                    class="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) opacity-0 transition-opacity hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) group-hover:opacity-100 focus-visible:opacity-100"
                    :disabled="
                      props.isUpdating ||
                      reorderablePlaylists.findIndex(
                        (candidate) => candidate.id === playlist.id,
                      ) ===
                        reorderablePlaylists.length - 1
                    "
                    type="button"
                    @click.stop="movePlaylist(playlist.id, 1)"
                  >
                    <span aria-hidden="true">↓</span>
                  </button>
                  <button
                    :aria-label="`Edit ${playlist.name}`"
                    class="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) opacity-0 transition-opacity hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) group-hover:opacity-100 focus-visible:opacity-100 [&>svg]:size-3.5"
                    type="button"
                    @click="emit('editPlaylist', playlist)"
                  >
                    <Pencil aria-hidden="true" />
                  </button>
                </ReorderItem>
              </ContextMenuTrigger>
              <ContextMenuContent data-playlist-context-menu>
                <ContextMenuItem @select="emit('playPlaylist', playlist)">
                  Play playlist
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem @select="emit('editPlaylist', playlist)">
                  Edit playlist
                </ContextMenuItem>
                <ContextMenuItem @select="emit('deletePlaylist', playlist)">
                  Delete playlist…
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          </ReorderGroup>
          <form
            v-if="props.isCreatingPlaylist"
            class="-mx-4 flex min-h-8 w-[calc(100%+2rem)] items-center gap-2 rounded-none bg-[oklch(0.7_0.03_262/0.15)] px-6.5"
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
        </nav>
      </div>
    </ScrollArea>
  </aside>
</template>

<style scoped>
/* The sidebar has no component-specific rules beyond utility classes. */
</style>
