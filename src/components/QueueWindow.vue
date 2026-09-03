<script setup lang="ts">
import {
  ListMusic,
  LoaderCircle,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Volume2,
} from "lucide-vue-next";
import {
  observeElementRect,
  type Rect,
  useVirtualizer,
  type Virtualizer,
} from "@tanstack/vue-virtual";
import { ReorderGroup, ReorderItem } from "motion-v";
import { type ComponentPublicInstance, computed, ref, watch } from "vue";

import type { MediaItem, PlaybackStatus } from "@/api";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatDuration } from "@/lib/time";
import YouTubeArtwork from "./YouTubeArtwork.vue";

interface Props {
  queue: MediaItem[];
  currentItemId: string | undefined;
  status: PlaybackStatus;
  positionMs: number;
  isStarting: boolean;
  isUpdating: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  playTrack: [id: string];
  move: [from: number, to: number];
}>();

const currentItem = computed(
  () => props.queue.find((item) => item.id === props.currentItemId) ?? null,
);
const isPlaying = computed(() => props.status === "playing");
const queueStartIndex = computed(() => {
  const index = props.queue.findIndex(
    (item) => item.id === props.currentItemId,
  );

  return index >= 0 ? index : 0;
});
const visibleQueue = computed(() => props.queue.slice(queueStartIndex.value));
const totalDurationMs = computed(() =>
  visibleQueue.value.reduce((total, item) => total + item.durationMs, 0),
);
const queueSummary = computed(() => {
  const count = visibleQueue.value.length;
  return `${count} ${count === 1 ? "track" : "tracks"} · ${formatDuration(totalDurationMs.value)}`;
});
const queueList = ref<HTMLElement | null>(null);
const reorderQueue = ref<MediaItem[]>([...visibleQueue.value]);
const activeReorderId = ref<string | null>(null);
const isReorderPending = ref(false);
const queueRowHeight = 52;
const queueReorderTransition = {
  damping: 42,
  stiffness: 650,
  type: "spring" as const,
};

function viewportHeight(): number {
  return typeof window === "undefined" ? 600 : window.innerHeight || 600;
}

function setQueueList(element: Element | ComponentPublicInstance | null): void {
  queueList.value = element instanceof HTMLElement ? element : null;
}

function observeQueueListRect(
  instance: Virtualizer<HTMLElement, Element>,
  callback: (rect: Rect) => void,
): (() => void) | undefined {
  return observeElementRect(instance, (rect) => {
    callback(rect.height > 0 ? rect : { ...rect, height: viewportHeight() });
  });
}

const virtualizerOptions = computed(() => {
  const scrollElement = queueList.value;

  return {
    count: reorderQueue.value.length,
    estimateSize: () => queueRowHeight,
    getScrollElement: () => scrollElement ?? queueList.value,
    initialRect: {
      height: viewportHeight(),
      width: 0,
    },
    observeElementRect: observeQueueListRect,
    overscan: 8,
  };
});
const queueVirtualizer = useVirtualizer(virtualizerOptions);
const virtualQueueItems = computed(() =>
  queueVirtualizer.value.getVirtualItems().flatMap((virtualItem) => {
    const item = reorderQueue.value[virtualItem.index];

    return item ? [{ item, virtualItem }] : [];
  }),
);
const virtualQueueHeight = computed(
  () => `${queueVirtualizer.value.getTotalSize()}px`,
);

function playTrack(id: string): void {
  if (!props.isUpdating && !props.isStarting) {
    emit("playTrack", id);
  }
}

function startQueueReorder(itemId: string): void {
  activeReorderId.value = itemId;
}

function previewQueueReorder(items: MediaItem[]): void {
  reorderQueue.value = items;
}

function finishQueueReorder(): void {
  const itemId = activeReorderId.value;
  activeReorderId.value = null;

  if (!itemId || props.isUpdating || props.isStarting) {
    reorderQueue.value = [...visibleQueue.value];
    return;
  }

  const from = props.queue.findIndex((item) => item.id === itemId);
  const to =
    queueStartIndex.value +
    reorderQueue.value.findIndex((item) => item.id === itemId);

  if (from === -1 || to === -1 || from === to) {
    reorderQueue.value = [...visibleQueue.value];
    return;
  }

  isReorderPending.value = true;
  emit("move", from, to);
}

watch([() => props.queue, () => props.currentItemId], () => {
  reorderQueue.value = [...visibleQueue.value];
  activeReorderId.value = null;
  isReorderPending.value = false;
});

watch(
  () => props.isUpdating,
  (isUpdating) => {
    if (!isUpdating && isReorderPending.value) {
      reorderQueue.value = [...visibleQueue.value];
      isReorderPending.value = false;
    }
  },
);
</script>

<template>
  <main
    class="queue-window window-shell window-surface flex h-screen min-h-0 flex-col text-(--text)"
    aria-label="Play queue"
  >
    <div class="window-drag-region" data-tauri-drag-region aria-hidden="true" />
    <header class="window-header shrink-0 px-4 pt-8 pb-2">
      <h1 class="window-title">Play queue</h1>
      <p
        class="mt-1 text-[0.77rem] text-(--muted-text) tabular-nums"
        data-queue-summary
      >
        {{ queueSummary }}
      </p>
    </header>

    <section
      class="flex min-h-0 flex-1 flex-col"
      aria-labelledby="queue-list-heading"
    >
      <h2 id="queue-list-heading" class="sr-only">Queue tracks</h2>

      <div
        v-if="visibleQueue.length === 0"
        class="grid min-h-0 flex-1 place-items-center px-6 text-center"
      >
        <div class="max-w-52">
          <ListMusic
            class="mx-auto size-5 text-(--subtle-text)"
            aria-hidden="true"
          />
          <p class="mt-3 text-sm font-[650]">Your queue is empty</p>
          <p class="mt-1 text-xs leading-5 text-(--muted-text)">
            Import music from the Library to start listening.
          </p>
        </div>
      </div>

      <ScrollArea
        v-else
        class="min-h-0 flex-1"
        data-queue-scroll
        type="scroll"
        viewport-class="queue-scroll"
        :viewport-ref="setQueueList"
      >
        <ReorderGroup
          as="div"
          axis="y"
          class="relative min-w-0"
          data-queue-virtualizer
          role="list"
          :style="{ height: virtualQueueHeight }"
          :values="reorderQueue"
          @update:values="previewQueueReorder"
        >
          <div
            v-for="{ item, virtualItem } in virtualQueueItems"
            :key="item.id"
            class="absolute left-0 w-full"
            :style="{ transform: `translateY(${virtualItem.start}px)` }"
          >
            <ReorderItem
              as="article"
              :drag="
                isUpdating || isStarting || item.id === currentItemId
                  ? false
                  : 'y'
              "
              :drag-momentum="false"
              :on-drag-end="finishQueueReorder"
              :on-drag-start="() => startQueueReorder(item.id)"
              :transition="queueReorderTransition"
              :value="item"
              :aria-current="item.id === currentItemId ? 'true' : undefined"
              :aria-label="`${item.title} by ${item.artist}`"
              :data-current="item.id === currentItemId"
              :data-queue-index="virtualItem.index"
              :data-queue-item-id="item.id"
              class="queue-row group grid h-13 w-full cursor-grab grid-cols-[2.25rem_minmax(0,1fr)_3.5rem_3.25rem] items-center border-b border-(--line) outline-none active:cursor-grabbing hover:bg-[oklch(0.72_0.025_258/0.08)] data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
              data-queue-item
              role="listitem"
            >
              <div class="grid place-items-center">
                <Button
                  :aria-label="`Play ${item.title}`"
                  :class="
                    item.id === currentItemId
                      ? 'text-accent'
                      : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100'
                  "
                  size="icon-xs"
                  variant="ghost"
                  :disabled="isUpdating || isStarting"
                  @click="playTrack(item.id)"
                >
                  <Volume2
                    v-if="item.id === currentItemId"
                    aria-hidden="true"
                  />
                  <Play v-else aria-hidden="true" fill="currentColor" />
                </Button>
              </div>
              <div class="min-w-0 pr-2">
                <p
                  class="overflow-hidden text-[0.8rem] font-medium text-ellipsis whitespace-nowrap"
                >
                  {{ item.title }}
                </p>
                <p
                  class="mt-0.5 overflow-hidden text-[0.68rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
                >
                  {{ item.artist }}
                </p>
              </div>
              <span
                class="text-center text-[0.72rem] text-(--muted-text) tabular-nums"
              >
                {{ formatDuration(item.durationMs) }}
              </span>
              <div
                class="flex items-center justify-end gap-0.5 pr-2 text-(--subtle-text)"
              >
                <Button
                  :aria-label="`Move ${item.title} up`"
                  class="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100"
                  size="icon-xs"
                  variant="ghost"
                  :disabled="
                    isUpdating ||
                    item.id === currentItemId ||
                    virtualItem.index === 0
                  "
                  @click.stop="
                    emit(
                      'move',
                      queueStartIndex + virtualItem.index,
                      queueStartIndex + virtualItem.index - 1,
                    )
                  "
                >
                  <span aria-hidden="true">↑</span>
                </Button>
                <Button
                  :aria-label="`Move ${item.title} down`"
                  class="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100"
                  size="icon-xs"
                  variant="ghost"
                  :disabled="
                    isUpdating ||
                    item.id === currentItemId ||
                    virtualItem.index === visibleQueue.length - 1
                  "
                  @click.stop="
                    emit(
                      'move',
                      queueStartIndex + virtualItem.index,
                      queueStartIndex + virtualItem.index + 1,
                    )
                  "
                >
                  <span aria-hidden="true">↓</span>
                </Button>
              </div>
            </ReorderItem>
          </div>
        </ReorderGroup>
      </ScrollArea>
    </section>

    <footer
      v-if="currentItem"
      class="grid shrink-0 grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 border-t border-(--line) p-2"
      data-queue-footer
    >
      <div class="queue-cover size-12 -m-2 shrink-0">
        <YouTubeArtwork
          class="absolute inset-0 size-full object-cover"
          :video-id="currentItem.id"
        />
      </div>
      <div class="min-w-0">
        <p
          class="overflow-hidden text-[0.78rem] font-semibold text-ellipsis whitespace-nowrap"
        >
          {{ currentItem.title }}
        </p>
        <p
          class="mt-0.5 overflow-hidden text-[0.67rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
        >
          {{ currentItem.artist }} · {{ formatDuration(positionMs) }} /
          {{ formatDuration(currentItem.durationMs) }}
        </p>
      </div>
      <nav
        class="flex shrink-0 items-center gap-0.5"
        aria-label="Queue playback controls"
      >
        <Button
          aria-label="Previous track"
          size="icon-sm"
          variant="ghost"
          :disabled="isUpdating"
          @click="emit('previous')"
        >
          <SkipBack aria-hidden="true" />
        </Button>
        <Button
          :aria-busy="isStarting ? 'true' : undefined"
          :aria-label="
            isStarting
              ? 'Starting playback'
              : isPlaying
                ? 'Pause playback'
                : 'Play playback'
          "
          class="rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text)"
          size="icon-sm"
          :disabled="isUpdating"
          @click="emit('toggle')"
        >
          <LoaderCircle
            v-if="isStarting"
            class="animate-spin"
            aria-hidden="true"
          />
          <Pause v-else-if="isPlaying" aria-hidden="true" fill="currentColor" />
          <Play v-else aria-hidden="true" fill="currentColor" />
        </Button>
        <Button
          aria-label="Next track"
          size="icon-sm"
          variant="ghost"
          :disabled="isUpdating"
          @click="emit('next')"
        >
          <SkipForward aria-hidden="true" />
        </Button>
      </nav>
    </footer>
  </main>
</template>

<style scoped>
.queue-cover {
  position: relative;
  overflow: hidden;
  background: linear-gradient(
    138deg,
    var(--artwork-a),
    var(--artwork-b) 58%,
    var(--artwork-c)
  );
}

.queue-scroll {
  scrollbar-gutter: stable;
}
</style>
