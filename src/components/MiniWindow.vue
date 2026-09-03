<script setup lang="ts">
import { ChevronDown, ChevronUp, Palette, X } from "lucide-vue-next";

import type { PlaybackSnapshot } from "@/api";
import MiniPlayer from "@/components/MiniPlayer.vue";
import QueueDrawer from "@/components/QueueDrawer.vue";
import { Button } from "@/components/ui/button";
import type { ThemeName } from "@/lib/theme";

interface Props {
  snapshot: PlaybackSnapshot;
  isUpdating: boolean;
  isStarting?: boolean;
  favoriteTrackIds?: string[];
  queueExpanded: boolean;
  theme: ThemeName;
}

const props = withDefaults(defineProps<Props>(), {
  favoriteTrackIds: () => [],
  isStarting: false,
});

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [volumePercent: number];
  toggleMute: [];
  move: [from: number, to: number];
  remove: [index: number];
  toggleQueue: [];
  toggleTheme: [];
  close: [];
  toggleShuffle: [];
  cycleRepeatMode: [];
  toggleFavorite: [id: string];
}>();
</script>

<template>
  <main class="mini-window-shell min-h-36 w-full">
    <section
      class="player-frame window-panel relative isolate overflow-hidden rounded-[18px] bg-(--glass-window) text-(--text) shadow-2xl backdrop-blur-xl backdrop-saturate-[1.18]"
      :class="queueExpanded ? 'min-h-97.5' : ''"
      :data-queue-expanded="queueExpanded"
    >
      <MiniPlayer
        :snapshot="snapshot"
        :is-updating="isUpdating"
        :is-starting="props.isStarting"
        :favorite-track-ids="props.favoriteTrackIds"
        @toggle="emit('toggle')"
        @previous="emit('previous')"
        @next="emit('next')"
        @seek="emit('seek', $event)"
        @set-volume="emit('setVolume', $event)"
        @toggle-mute="emit('toggleMute')"
        @toggle-shuffle="emit('toggleShuffle')"
        @cycle-repeat-mode="emit('cycleRepeatMode')"
        @toggle-favorite="emit('toggleFavorite', $event)"
      />
      <QueueDrawer
        v-if="queueExpanded"
        :queue="snapshot.queue"
        :current-item-id="snapshot.currentItem?.id"
        :is-updating="isUpdating"
        @move="(from, to) => emit('move', from, to)"
        @remove="emit('remove', $event)"
      />
      <nav
        class="absolute top-1.5 right-2 z-5 flex items-center gap-0.5 [&_button]:text-(--subtle-text) [&_button:hover]:bg-(--surface-muted) [&_button:hover]:text-(--text)"
        aria-label="Mini player options"
      >
        <Button
          aria-label="Change theme"
          size="icon-xs"
          variant="ghost"
          @click="emit('toggleTheme')"
          ><Palette aria-hidden="true"
        /></Button>
        <Button
          :aria-label="queueExpanded ? 'Hide queue' : 'Show queue'"
          size="icon-xs"
          variant="ghost"
          @click="emit('toggleQueue')"
        >
          <ChevronUp v-if="queueExpanded" aria-hidden="true" />
          <ChevronDown v-else aria-hidden="true" />
        </Button>
        <Button
          aria-label="Close mini player"
          size="icon-xs"
          variant="ghost"
          @click="emit('close')"
          ><X aria-hidden="true"
        /></Button>
      </nav>
    </section>
  </main>
</template>

<style scoped>
.player-frame::before {
  position: absolute;
  z-index: 4;
  top: 0;
  right: 0;
  left: 0;
  height: 1px;
  content: "";
  background: linear-gradient(
    90deg,
    transparent,
    var(--line-strong),
    transparent
  );
  opacity: 0.7;
  pointer-events: none;
}

.player-frame::after {
  position: absolute;
  z-index: 4;
  inset: 1px;
  border: 1px solid color-mix(in oklch, var(--line-strong), transparent 58%);
  border-radius: inherit;
  content: "";
  pointer-events: none;
}
</style>
