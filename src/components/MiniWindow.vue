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
  queueExpanded: boolean;
  theme: ThemeName;
}

defineProps<Props>();

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [volumePercent: number];
  toggleMute: [];
  move: [from: number, to: number];
  toggleQueue: [];
  toggleTheme: [];
  close: [];
}>();
</script>

<template>
  <main class="mini-window-shell min-h-43.5 w-full">
    <section
      class="player-frame relative overflow-hidden rounded-[22px] bg-(--glass-panel) text-(--text) backdrop-saturate-[1.18]"
      :class="queueExpanded ? 'min-h-105' : ''"
      :data-queue-expanded="queueExpanded"
    >
      <MiniPlayer
        :snapshot="snapshot"
        :is-updating="isUpdating"
        @toggle="emit('toggle')"
        @previous="emit('previous')"
        @next="emit('next')"
        @seek="emit('seek', $event)"
        @set-volume="emit('setVolume', $event)"
        @toggle-mute="emit('toggleMute')"
      />
      <QueueDrawer
        v-if="queueExpanded"
        :queue="snapshot.queue"
        :current-item-id="snapshot.currentItem?.id"
        :is-updating="isUpdating"
        @move="(from, to) => emit('move', from, to)"
      />
      <nav
        class="absolute top-2 right-2 z-3 grid gap-0.5 [&_button]:text-(--subtle-text) [&_button:hover]:bg-(--surface-muted) [&_button:hover]:text-(--text)"
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
</style>
