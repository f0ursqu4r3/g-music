<script setup lang="ts">
import { GripVertical, MoreHorizontal } from "lucide-vue-next";

import type { MediaItem } from "@/api";
import { Button } from "@/components/ui/button";
import { formatDuration } from "@/lib/time";

interface Props {
  queue: MediaItem[];
  currentItemId: string | undefined;
}

defineProps<Props>();
</script>

<template>
  <main
    class="queue-window window-shell window-surface px-4.5 pb-5.5"
    aria-labelledby="queue-window-heading"
  >
    <header class="window-header items-end gap-5 px-2 pt-6 pb-4">
      <div>
        <p class="window-kicker text-[0.58rem] font-[720]">Listening order</p>
        <h1
          id="queue-window-heading"
          class="window-title mt-0.75 font-bold tracking-[-0.045em]"
        >
          Play queue
        </h1>
      </div>
      <Button aria-label="More queue actions" size="icon-sm" variant="ghost"
        ><MoreHorizontal aria-hidden="true"
      /></Button>
    </header>

    <ol class="grid list-none gap-1 pt-4">
      <li
        v-for="(item, index) in queue"
        :key="item.id"
        class="grid min-h-13.5 grid-cols-[26px_38px_minmax(0,1fr)_36px_16px] items-center gap-2.5 rounded-[9px] px-2.25 py-1.75 hover:bg-(--surface-muted) data-[current=true]:bg-(--accent-soft) data-[current=true]:outline data-[current=true]:outline-[color-mix(in_oklch,var(--accent)_36%,transparent)]"
        :data-current="item.id === currentItemId"
      >
        <span class="text-[0.64rem] text-(--subtle-text) tabular-nums">{{
          String(index + 1).padStart(2, "0")
        }}</span>
        <span
          class="queue-cover relative size-9.5 overflow-hidden rounded-md"
          :data-cover="index % 2 === 0 ? 'violet' : 'cyan'"
        />
        <div class="min-w-0">
          <p
            class="overflow-hidden text-[0.76rem] font-[640] text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ item.title }}
          </p>
          <span
            class="mt-0.75 block overflow-hidden text-[0.68rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
            >{{ item.artist }}</span
          >
        </div>
        <time
          class="text-right text-[0.64rem] text-(--subtle-text) tabular-nums"
          >{{ formatDuration(item.durationMs) }}</time
        >
        <GripVertical
          class="size-3.75 text-(--subtle-text)"
          aria-hidden="true"
        />
      </li>
    </ol>
  </main>
</template>

<style scoped>
.queue-cover {
  background: linear-gradient(
    138deg,
    var(--artwork-a),
    var(--artwork-b) 58%,
    var(--artwork-c)
  );
}

.queue-cover::after {
  position: absolute;
  inset: 15%;
  content: "";
  border: 1px solid oklch(0.98 0.01 90 / 0.35);
  border-radius: inherit;
  transform: rotate(-18deg);
}
</style>
