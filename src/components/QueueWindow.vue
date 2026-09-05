<script setup lang="ts">
import {
  Disc3,
  ListMusic,
  LoaderCircle,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Trash2,
  Volume2,
} from "lucide-vue-next";
import {
  observeElementRect,
  type Rect,
  useVirtualizer,
  type Virtualizer,
} from "@tanstack/vue-virtual";
import { ReorderGroup, ReorderItem } from "motion-v";
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
  DialogTrigger,
} from "reka-ui";
import { type ComponentPublicInstance, computed, ref, watch } from "vue";

import type { MediaItem, PlaybackStatus, Playlist } from "@/api";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatDuration } from "@/lib/time";
import YouTubeArtwork from "./YouTubeArtwork.vue";

interface Props {
  queue: MediaItem[];
  playbackOrder?: string[];
  currentItemId: string | undefined;
  status: PlaybackStatus;
  shuffleEnabled?: boolean;
  positionMs: number;
  isStarting: boolean;
  isUpdating: boolean;
  clearQueue?: () => Promise<unknown>;
  savePlaylist?: (playlist: Playlist) => Promise<unknown>;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  playTrack: [id: string];
  move: [from: number, to: number];
  remove: [index: number];
}>();

const displayQueue = computed(() => {
  if (
    !props.shuffleEnabled ||
    !props.playbackOrder ||
    props.playbackOrder.length !== props.queue.length ||
    new Set(props.playbackOrder).size !== props.queue.length
  ) {
    return props.queue;
  }

  const itemsById = new Map(props.queue.map((item) => [item.id, item]));
  const orderedQueue: MediaItem[] = [];
  for (const id of props.playbackOrder) {
    const item = itemsById.get(id);
    if (!item) {
      return props.queue;
    }
    orderedQueue.push(item);
  }

  return orderedQueue;
});
const isShuffled = computed(() => displayQueue.value !== props.queue);
const currentItem = computed(
  () =>
    displayQueue.value.find((item) => item.id === props.currentItemId) ?? null,
);
const isPlaying = computed(() => props.status === "playing");
const clearOpen = ref(false);
const saveOpen = ref(false);
const actionPending = ref(false);
const actionError = ref("");
const playlistName = ref("");
const playlistId = ref("");
const playlistTrackIds = ref<string[]>([]);
const actionDisabled = computed(
  () =>
    props.queue.length === 0 ||
    props.isUpdating ||
    props.isStarting ||
    actionPending.value,
);

function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error)
    return String(error.message);
  return String(error);
}

function openSave(open: boolean): void {
  if (actionPending.value) return;
  if (open) {
    actionError.value = "";
    playlistId.value = `playlist-${crypto.randomUUID()}`;
    playlistTrackIds.value = displayQueue.value.map((item) => item.id);
  }
  saveOpen.value = open;
}

function openClear(open: boolean): void {
  if (actionPending.value) return;
  actionError.value = "";
  clearOpen.value = open;
}

async function clear(): Promise<void> {
  if (!props.clearQueue || actionPending.value) return;
  actionPending.value = true;
  actionError.value = "";
  try {
    await props.clearQueue();
    clearOpen.value = false;
  } catch (error) {
    actionError.value = `${errorMessage(error)}. Try clearing the queue again.`;
  } finally {
    actionPending.value = false;
  }
}

async function save(): Promise<void> {
  const name = playlistName.value.trim();
  if (!name || !props.savePlaylist || actionPending.value) return;
  actionPending.value = true;
  actionError.value = "";
  try {
    await props.savePlaylist({
      id: playlistId.value,
      name,
      trackIds: [...playlistTrackIds.value],
    });
    saveOpen.value = false;
    playlistName.value = "";
  } catch (error) {
    actionError.value = `${errorMessage(error)}. Your playlist draft is kept. Try Save playlist again.`;
  } finally {
    actionPending.value = false;
  }
}
const queueStartIndex = computed(() => {
  const index = displayQueue.value.findIndex(
    (item) => item.id === props.currentItemId,
  );

  return index >= 0 ? index : 0;
});
const visibleQueue = computed(() =>
  displayQueue.value.slice(queueStartIndex.value),
);
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

function queueIndex(id: string): number {
  return props.queue.findIndex((item) => item.id === id);
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

  if (!itemId || props.isUpdating || props.isStarting || isShuffled.value) {
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

watch(
  [
    () => props.queue,
    () => props.currentItemId,
    () => props.playbackOrder,
    () => props.shuffleEnabled,
  ],
  () => {
    reorderQueue.value = [...visibleQueue.value];
    activeReorderId.value = null;
    isReorderPending.value = false;
  },
);

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

    <div
      class="flex shrink-0 items-center gap-2 border-b border-(--line) px-4 py-2"
      aria-label="Queue actions"
    >
      <DialogRoot :open="saveOpen" @update:open="openSave">
        <DialogTrigger as-child>
          <Button
            aria-label="Save queue as playlist"
            size="sm"
            variant="outline"
            :disabled="actionDisabled || !savePlaylist"
            >Save as playlist</Button
          >
        </DialogTrigger>
        <DialogPortal>
          <DialogOverlay class="fixed inset-0 z-50 bg-black/60" />
          <DialogContent
            class="queue-action-dialog fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100dvh-2rem)] w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-(--line-strong) bg-(--surface) p-5 text-(--text) shadow-xl"
            @escape-key-down="actionPending && $event.preventDefault()"
            @interact-outside.prevent
          >
            <DialogTitle class="text-lg font-semibold"
              >Save queue as playlist</DialogTitle
            >
            <DialogDescription class="mt-1 text-sm text-(--muted-text)"
              >Save all {{ playlistTrackIds.length }} queue tracks in their
              current playback order.</DialogDescription
            >
            <form
              class="mt-4 flex min-h-0 flex-col gap-4"
              @submit.prevent="save"
            >
              <div class="min-h-0 overflow-y-auto">
                <label
                  for="queue-playlist-name"
                  class="block text-sm font-medium"
                  >Playlist name</label
                >
                <input
                  id="queue-playlist-name"
                  v-model="playlistName"
                  :disabled="actionPending"
                  required
                  maxlength="200"
                  class="mt-1 w-full rounded-md border border-(--line-strong) bg-(--glass-control) px-3 py-2 outline-offset-2"
                />
                <p
                  v-if="actionError"
                  role="alert"
                  class="window-alert-danger mt-3 rounded-md p-3 text-sm wrap-break-word"
                >
                  {{ actionError }}
                </p>
              </div>
              <div class="flex shrink-0 justify-end gap-2">
                <Button
                  type="button"
                  variant="outline"
                  :disabled="actionPending"
                  @click="openSave(false)"
                  >Cancel</Button
                >
                <Button
                  type="submit"
                  :disabled="!playlistName.trim() || actionPending"
                >
                  {{ actionPending ? "Saving…" : "Save playlist" }}
                </Button>
              </div>
            </form>
          </DialogContent>
        </DialogPortal>
      </DialogRoot>
      <DialogRoot :open="clearOpen" @update:open="openClear">
        <DialogTrigger as-child>
          <Button
            aria-label="Clear queue"
            size="sm"
            variant="ghost"
            :disabled="actionDisabled || !clearQueue"
            >Clear queue</Button
          >
        </DialogTrigger>
        <DialogPortal>
          <DialogOverlay class="fixed inset-0 z-50 bg-black/60" />
          <DialogContent
            class="queue-action-dialog fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100dvh-2rem)] w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-(--line-strong) bg-(--surface) p-5 text-(--text) shadow-xl"
            @escape-key-down="actionPending && $event.preventDefault()"
            @interact-outside.prevent
          >
            <DialogTitle class="text-lg font-semibold"
              >Clear queue?</DialogTitle
            >
            <div class="min-h-0 overflow-y-auto">
              <DialogDescription class="mt-2 text-sm text-(--muted-text)"
                >This will stop playback and remove all tracks from the queue.
                Your library and playlists are kept.</DialogDescription
              >
              <p
                v-if="actionError"
                role="alert"
                class="window-alert-danger mt-3 rounded-md p-3 text-sm wrap-break-word"
              >
                {{ actionError }}
              </p>
            </div>
            <div class="mt-4 flex shrink-0 flex-wrap justify-end gap-2">
              <Button
                variant="outline"
                :disabled="actionPending"
                @click="openClear(false)"
                >Cancel</Button
              >
              <Button :disabled="actionPending" @click="clear">{{
                actionPending ? "Clearing…" : "Stop and clear queue"
              }}</Button>
            </div>
          </DialogContent>
        </DialogPortal>
      </DialogRoot>
    </div>

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
            <ContextMenu>
              <ContextMenuTrigger as-child>
                <ReorderItem
                  as="article"
                  :drag="
                    isShuffled ||
                    isUpdating ||
                    isStarting ||
                    item.id === currentItemId
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
                  class="queue-row group grid h-13 w-full cursor-grab grid-cols-[2.25rem_minmax(0,1fr)_max-content_2.25rem] items-center border-b border-(--line) outline-none active:cursor-grabbing hover:bg-[oklch(0.72_0.025_258/0.08)] data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
                  data-queue-item
                  role="listitem"
                >
                  <div
                    class="grid place-items-center"
                    :class="
                      item.id !== currentItemId
                        ? 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100'
                        : undefined
                    "
                  >
                    <Button
                      :aria-label="`Play ${item.title}`"
                      :class="{ 'text-accent': item.id === currentItemId }"
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
                    class="pr-2 text-right text-[0.72rem] whitespace-nowrap text-(--muted-text) tabular-nums"
                    data-queue-duration
                  >
                    {{ formatDuration(item.durationMs) }}
                  </span>
                  <div
                    class="flex items-center justify-end gap-0.5 pr-2 text-(--subtle-text) opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
                  >
                    <Button
                      v-if="item.id !== currentItemId"
                      :aria-label="`Remove ${item.title} from queue`"
                      size="icon-xs"
                      variant="ghost"
                      :disabled="isUpdating || isStarting"
                      @click.stop="emit('remove', queueIndex(item.id))"
                    >
                      <Trash2 aria-hidden="true" />
                    </Button>
                  </div>
                </ReorderItem>
              </ContextMenuTrigger>
              <ContextMenuContent data-queue-context-menu>
                <ContextMenuItem
                  :disabled="
                    isUpdating || isStarting || item.id === currentItemId
                  "
                  @select="playTrack(item.id)"
                >
                  Play now
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                  :disabled="
                    isShuffled ||
                    isUpdating ||
                    isStarting ||
                    item.id === currentItemId ||
                    virtualItem.index === 0
                  "
                  @select="
                    emit(
                      'move',
                      queueStartIndex + virtualItem.index,
                      queueStartIndex + virtualItem.index - 1,
                    )
                  "
                >
                  Move up
                </ContextMenuItem>
                <ContextMenuItem
                  :disabled="
                    isShuffled ||
                    isUpdating ||
                    isStarting ||
                    item.id === currentItemId ||
                    virtualItem.index === visibleQueue.length - 1
                  "
                  @select="
                    emit(
                      'move',
                      queueStartIndex + virtualItem.index,
                      queueStartIndex + virtualItem.index + 1,
                    )
                  "
                >
                  Move down
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                  v-if="item.id !== currentItemId"
                  variant="destructive"
                  :disabled="isUpdating || isStarting"
                  @select="emit('remove', queueIndex(item.id))"
                >
                  Remove from queue
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
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
          :missing-icon="Disc3"
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
          :disabled="isUpdating || isStarting"
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
          :disabled="isUpdating || isStarting"
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
          :disabled="isUpdating || isStarting"
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
