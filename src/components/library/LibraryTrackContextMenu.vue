<script setup lang="ts">
import { computed } from "vue";

import type { MediaItem } from "@/api";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";

const props = defineProps<{
  canRemoveFromPlaylist?: boolean;
  isUpdating?: boolean;
  isFavorite: boolean;
  selectedTracks: MediaItem[];
  track: MediaItem;
}>();

const emit = defineEmits<{
  addToQueue: [tracks: MediaItem[]];
  edit: [track: MediaItem];
  openAlbum: [track: MediaItem];
  openArtist: [track: MediaItem];
  open: [track: MediaItem];
  play: [tracks: MediaItem[]];
  playNext: [tracks: MediaItem[]];
  remove: [tracks: MediaItem[]];
  removeFromPlaylist: [tracks: MediaItem[]];
  toggleFavorite: [ids: string[]];
}>();

const menuTracks = computed(() =>
  props.selectedTracks.some((track) => track.id === props.track.id)
    ? props.selectedTracks
    : [props.track],
);
const isMultiple = computed(() => menuTracks.value.length > 1);
const trackCountLabel = computed(
  () =>
    `${menuTracks.value.length} ${menuTracks.value.length === 1 ? "track" : "tracks"}`,
);

function selectMenuTarget(isOpen: boolean): void {
  if (isOpen) {
    emit("open", props.track);
  }
}
</script>

<template>
  <ContextMenu @update:open="selectMenuTarget">
    <ContextMenuTrigger as-child>
      <slot />
    </ContextMenuTrigger>
    <ContextMenuContent data-track-context-menu>
      <ContextMenuItem
        data-track-context-action="play"
        @select="emit('play', menuTracks)"
      >
        {{ isMultiple ? `Play ${trackCountLabel}` : "Play" }}
      </ContextMenuItem>
      <ContextMenuItem
        data-track-context-action="play-next"
        @select="emit('playNext', menuTracks)"
      >
        {{ isMultiple ? `Play ${trackCountLabel} next` : "Play next" }}
      </ContextMenuItem>
      <ContextMenuItem
        data-track-context-action="add-to-queue"
        @select="emit('addToQueue', menuTracks)"
      >
        {{ isMultiple ? `Add ${trackCountLabel} to queue` : "Add to queue" }}
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        data-track-context-action="favorite"
        @select="
          emit(
            'toggleFavorite',
            menuTracks.map((track) => track.id),
          )
        "
      >
        {{
          isMultiple
            ? "Toggle Favorites"
            : props.isFavorite
              ? "Remove from Favorites"
              : "Add to Favorites"
        }}
      </ContextMenuItem>
      <ContextMenuItem
        v-if="!isMultiple && props.track.album"
        data-track-context-action="open-album"
        @select="emit('openAlbum', props.track)"
      >
        Go to album
      </ContextMenuItem>
      <ContextMenuItem
        v-if="!isMultiple"
        data-track-context-action="open-artist"
        @select="emit('openArtist', props.track)"
      >
        Go to artist
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        data-track-context-action="edit"
        @select="emit('edit', props.track)"
      >
        Edit metadata
      </ContextMenuItem>
      <ContextMenuItem
        v-if="props.canRemoveFromPlaylist"
        :disabled="props.isUpdating"
        data-track-context-action="remove-from-playlist"
        @select="emit('removeFromPlaylist', menuTracks)"
      >
        {{
          isMultiple
            ? `Remove ${trackCountLabel} from playlist`
            : "Remove from playlist"
        }}
      </ContextMenuItem>
      <ContextMenuItem
        data-track-context-action="remove"
        variant="destructive"
        @select="emit('remove', menuTracks)"
      >
        {{
          isMultiple
            ? `Remove ${trackCountLabel} from library…`
            : "Remove from library…"
        }}
      </ContextMenuItem>
    </ContextMenuContent>
  </ContextMenu>
</template>
