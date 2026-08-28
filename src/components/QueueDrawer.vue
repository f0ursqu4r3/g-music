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
  <section
    class="border-t border-(--line) px-4 pt-3.25 pb-4.5"
    aria-labelledby="queue-heading"
  >
    <header class="flex items-end justify-between">
      <div>
        <p
          class="text-[0.57rem] font-[720] tracking-widest text-(--subtle-text) uppercase"
        >
          Listening order
        </p>
        <h2
          id="queue-heading"
          class="mt-0.75 text-[0.91rem] font-[670] tracking-tight"
        >
          Up next
        </h2>
      </div>
      <span class="text-[0.67rem] text-(--muted-text) tabular-nums"
        >{{ queue.length }} items</span
      >
    </header>

    <ol class="grid list-none gap-0.75 pt-2.5">
      <li
        v-for="(item, index) in queue"
        :key="item.id"
        class="group grid min-h-10.75 grid-cols-[25px_minmax(0,1fr)_36px_55px] items-center gap-1.75 rounded-[10px] px-1.75 py-1.5 hover:bg-(--surface-muted) data-[current=true]:bg-(--accent-soft) data-[current=true]:outline data-[current=true]:outline-[color-mix(in_oklch,var(--accent)_38%,transparent)]"
        :aria-current="item.id === currentItemId ? 'true' : undefined"
        :data-current="item.id === currentItemId"
      >
        <span
          class="grid place-items-center text-[0.64rem] text-(--subtle-text) tabular-nums group-data-[current=true]:text-accent [&>svg]:size-3.5"
          aria-hidden="true"
        >
          <GripVertical v-if="item.id !== currentItemId" />
          <span v-else>{{ String(index + 1).padStart(2, "0") }}</span>
        </span>
        <div class="min-w-0">
          <p
            class="overflow-hidden text-xs font-[650] tracking-[-0.01em] text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ item.title }}
          </p>
          <p
            class="mt-0.5 overflow-hidden text-[0.66rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
          >
            {{ item.artist }}
          </p>
        </div>
        <span class="text-[0.66rem] text-(--subtle-text) tabular-nums">{{
          formatDuration(item.durationMs)
        }}</span>
        <div class="flex justify-end gap-px">
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
