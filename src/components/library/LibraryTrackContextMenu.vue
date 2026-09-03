<script setup lang="ts">
import type { MediaItem } from "@/api";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";

const props = defineProps<{
  isFavorite: boolean;
  track: MediaItem;
}>();

const emit = defineEmits<{
  addToQueue: [id: string];
  edit: [track: MediaItem];
  openAlbum: [track: MediaItem];
  openArtist: [track: MediaItem];
  play: [track: MediaItem];
  playNext: [id: string];
  remove: [track: MediaItem];
  select: [track: MediaItem];
  toggleFavorite: [id: string];
}>();

function selectMenuTarget(isOpen: boolean): void {
  if (isOpen) {
    emit("select", props.track);
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
        @select="emit('play', props.track)"
      >
        Play
      </ContextMenuItem>
      <ContextMenuItem
        data-track-context-action="play-next"
        @select="emit('playNext', props.track.id)"
      >
        Play next
      </ContextMenuItem>
      <ContextMenuItem
        data-track-context-action="add-to-queue"
        @select="emit('addToQueue', props.track.id)"
      >
        Add to queue
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        data-track-context-action="favorite"
        @select="emit('toggleFavorite', props.track.id)"
      >
        {{ props.isFavorite ? "Remove from Favorites" : "Add to Favorites" }}
      </ContextMenuItem>
      <ContextMenuItem
        v-if="props.track.album"
        data-track-context-action="open-album"
        @select="emit('openAlbum', props.track)"
      >
        Go to album
      </ContextMenuItem>
      <ContextMenuItem
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
        data-track-context-action="remove"
        variant="destructive"
        @select="emit('remove', props.track)"
      >
        Remove from library…
      </ContextMenuItem>
    </ContextMenuContent>
  </ContextMenu>
</template>
