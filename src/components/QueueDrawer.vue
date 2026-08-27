<script setup lang="ts">
import { ChevronDown, ChevronUp, GripVertical } from "lucide-vue-next";

import type { MediaItem } from "@/api";
import { Button } from "@/components/ui/button";
import { formatDuration } from "@/lib/time";

interface Props {
  queue: MediaItem[];
  currentItemId: string | undefined;
  isUpdating: boolean;
}

defineProps<Props>();

const emit = defineEmits<{
  move: [from: number, to: number];
}>();
</script>

<template>
  <section class="queue-drawer" aria-labelledby="queue-heading">
    <header class="queue-heading">
      <div>
        <p>Up next</p>
        <h2 id="queue-heading">Queue</h2>
      </div>
      <span>{{ queue.length }} tracks</span>
    </header>

    <ol class="queue-list">
      <li
        v-for="(item, index) in queue"
        :key="item.id"
        class="queue-item"
        :data-current="item.id === currentItemId"
      >
        <span class="queue-index" aria-hidden="true">
          <GripVertical v-if="item.id !== currentItemId" />
          <span v-else>{{ String(index + 1).padStart(2, "0") }}</span>
        </span>
        <div class="min-w-0">
          <p class="queue-title">{{ item.title }}</p>
          <p class="queue-artist">{{ item.artist }}</p>
        </div>
        <span class="queue-duration">{{
          formatDuration(item.durationMs)
        }}</span>
        <div class="queue-actions">
          <Button
            :aria-label="`Move ${item.title} up`"
            size="icon-xs"
            variant="ghost"
            :disabled="isUpdating || index === 0"
            @click="emit('move', index, index - 1)"
          >
            <ChevronUp aria-hidden="true" />
          </Button>
          <Button
            :aria-label="`Move ${item.title} down`"
            size="icon-xs"
            variant="ghost"
            :disabled="isUpdating || index === queue.length - 1"
            @click="emit('move', index, index + 1)"
          >
            <ChevronDown aria-hidden="true" />
          </Button>
        </div>
      </li>
    </ol>
  </section>
</template>
