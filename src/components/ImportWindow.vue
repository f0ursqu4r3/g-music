<script setup lang="ts">
import { ArrowDownToLine, TerminalSquare } from "lucide-vue-next";
import { computed, onBeforeUnmount, ref, watch } from "vue";

import {
  playbackApi,
  type ImportProgress,
  type MediaItem,
  type MetadataRefreshSnapshot,
} from "@/api";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatDuration } from "@/lib/time";

interface Props {
  isImporting: boolean;
  errorMessage?: string;
  progress?: ImportProgress | null;
  isCancelling?: boolean;
  metadataRefreshes?: MetadataRefreshSnapshot;
  isRetryingMetadata?: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  importYoutubeUrls: [urls: string[]];
  cancelImport: [runId: number];
  retry: [];
  retryMetadata: [];
}>();

const youtubeUrls = ref("");
const searchQuery = ref("");
const searchResults = ref<MediaItem[]>([]);
const searchError = ref("");
const isSearching = ref(false);
const hasSearched = ref(false);
let searchGeneration = 0;
let submittedUrls = "";
let lastImportSources: string[] = [];
const activeProgress = computed(
  () =>
    props.progress &&
    !["completed", "failed", "cancelled"].includes(props.progress.phase),
);

watch(searchQuery, () => {
  searchGeneration += 1;
  isSearching.value = false;
  hasSearched.value = false;
  searchResults.value = [];
  searchError.value = "";
});
onBeforeUnmount(() => {
  searchGeneration += 1;
});

async function searchYouTube(): Promise<void> {
  const query = searchQuery.value.trim();
  if (!query) return;
  const generation = ++searchGeneration;
  isSearching.value = true;
  searchError.value = "";
  searchResults.value = [];
  try {
    const results = await playbackApi.searchYouTube(query);
    if (generation !== searchGeneration) return;
    searchResults.value = results.slice(0, 20);
    hasSearched.value = true;
  } catch (error) {
    if (generation !== searchGeneration) return;
    searchError.value =
      typeof error === "object" && error !== null && "message" in error
        ? String(error.message)
        : "Search failed. Check your connection and retry.";
  } finally {
    if (generation === searchGeneration) isSearching.value = false;
  }
}

function importSearchResult(item: MediaItem): void {
  if (props.isImporting || activeProgress.value) return;
  submittedUrls = "";
  lastImportSources = [
    item.sourceUrl ||
      `https://www.youtube.com/watch?v=${encodeURIComponent(item.id)}`,
  ];
  emit("importYoutubeUrls", lastImportSources);
}

function retryImport(): void {
  if (props.isImporting || activeProgress.value) return;
  if (lastImportSources.length) emit("importYoutubeUrls", lastImportSources);
  else emit("retry");
}
const logs = ref<string[]>(["Ready. Add one YouTube URL per line."]);
let currentRunId: number | undefined;
const importSources = computed(() => [
  ...new Set(
    youtubeUrls.value
      .split(/\r?\n/)
      .map((url) => url.trim())
      .filter(Boolean),
  ),
]);
const progressPercent = computed(() => {
  const total = props.progress?.totalSources ?? 0;
  if (total === 0) {
    return 0;
  }

  return Math.round(((props.progress?.completedSources ?? 0) / total) * 100);
});
const progressValue = computed<number | undefined>(() => {
  const progress = props.progress;
  if (progress?.phase === "resolving" && progress.completedSources === 0) {
    return undefined;
  }

  return progressPercent.value;
});

watch(
  () => props.progress,
  (progress) => {
    if (!progress) {
      return;
    }

    if (progress.phase === "started" && currentRunId !== progress.runId) {
      currentRunId = progress.runId;
      logs.value = [];
    }

    if (
      progress.phase === "completed" &&
      submittedUrls &&
      youtubeUrls.value === submittedUrls
    ) {
      youtubeUrls.value = "";
      submittedUrls = "";
    }

    const entry = progress.message.trim();
    if (entry && logs.value[logs.value.length - 1] !== entry) {
      logs.value.push(entry);
    }
  },
  { immediate: true },
);

function submitYouTubeUrls(): void {
  if (
    props.isImporting ||
    activeProgress.value ||
    importSources.value.length === 0
  ) {
    return;
  }

  submittedUrls = youtubeUrls.value;
  lastImportSources = importSources.value;
  emit("importYoutubeUrls", importSources.value);
}
</script>

<template>
  <main
    class="import-window window-shell window-surface grid h-screen min-h-0"
    aria-label="Import music"
  >
    <div
      class="application-drag-region window-drag-region"
      data-tauri-drag-region
      aria-hidden="true"
    />

    <section class="grid min-h-0 min-w-0 grid-rows-[88px_minmax(0,1fr)]">
      <header class="window-header">
        <div>
          <h1 class="window-title">Import Music</h1>
          <p class="mt-1 window-copy">
            Add videos, playlists, albums, channels, or artist pages.
          </p>
        </div>
        <span class="window-status">
          {{ isImporting ? "Import running" : "Library import" }}
        </span>
      </header>

      <ScrollArea class="min-h-0 size-full" type="scroll">
        <div class="px-5 py-5">
          <p
            v-if="errorMessage"
            class="window-alert-danger mb-4 break-words px-3 py-2 text-sm"
            role="alert"
          >
            {{ errorMessage }}
          </p>

          <Button
            v-if="errorMessage"
            type="button"
            variant="outline"
            class="mb-4"
            @click="retryImport"
            :disabled="isImporting || Boolean(activeProgress)"
            >Retry</Button
          >

          <section
            class="window-panel mb-4 p-5"
            aria-labelledby="youtube-search-heading"
          >
            <h2 id="youtube-search-heading" class="text-base font-semibold">
              Discover on YouTube
            </h2>
            <p class="mt-1 text-sm text-(--muted-text)">
              Search by song, artist, or album. Import adds music to your
              library.
            </p>
            <form
              aria-label="Search YouTube"
              class="mt-3 flex gap-2"
              @submit.prevent="searchYouTube"
            >
              <input
                v-model="searchQuery"
                aria-label="Search YouTube"
                type="search"
                placeholder="Song, artist, or album"
                class="min-w-0 flex-1 rounded-lg border border-(--line-strong) bg-(--glass-control) px-3 py-2 text-sm outline-none focus:border-(--focus-ring)"
              />
              <Button
                type="submit"
                :disabled="!searchQuery.trim() || isSearching"
                >{{ isSearching ? "Searching…" : "Search" }}</Button
              >
            </form>
            <p
              v-if="isSearching"
              role="status"
              class="mt-3 text-sm text-(--muted-text)"
            >
              Searching YouTube…
            </p>
            <div v-if="searchError" class="window-alert-danger mt-3 p-3">
              <p role="alert" class="break-words text-sm">{{ searchError }}</p>
              <Button
                aria-label="Retry search"
                variant="outline"
                type="button"
                class="mt-2"
                @click="searchYouTube"
                >Retry search</Button
              >
            </div>
            <p
              v-else-if="hasSearched && !searchResults.length"
              role="status"
              class="mt-3 text-sm text-(--muted-text)"
            >
              No results. Try a different song or artist.
            </p>
            <p
              v-else-if="hasSearched"
              role="status"
              class="mt-3 text-xs text-(--muted-text)"
            >
              {{ searchResults.length }} results · Up to 20 shown
            </p>
            <ol
              v-if="searchResults.length"
              class="mt-2 max-h-80 overflow-y-auto divide-y divide-(--line)"
              aria-label="YouTube search results"
            >
              <li
                v-for="item in searchResults"
                :key="item.id"
                :data-search-result="item.id"
                class="flex items-center gap-3 py-3"
              >
                <div class="min-w-0 flex-1">
                  <p class="break-words text-sm font-medium">
                    {{ item.title }}
                  </p>
                  <p class="break-words text-xs text-(--muted-text)">
                    {{ item.artist
                    }}<template v-if="item.album"> · {{ item.album }}</template>
                    · {{ formatDuration(item.durationMs) }}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  :aria-label="`Import ${item.title}`"
                  :disabled="isImporting || Boolean(activeProgress)"
                  @click="importSearchResult(item)"
                  >Import</Button
                >
              </li>
            </ol>
          </section>

          <section
            v-if="metadataRefreshes?.totalTracks"
            aria-label="Metadata refresh"
            class="window-panel mb-4 p-4"
          >
            <h2 class="text-sm font-semibold">Metadata refresh</h2>
            <p class="mt-1 text-xs text-(--muted-text)">
              {{ metadataRefreshes.completedTracks }} of
              {{ metadataRefreshes.totalTracks }} refreshed
            </p>
            <template
              v-if="
                metadataRefreshes.jobs.some((job) => job.state === 'failed')
              "
            >
              <p class="mt-2 text-sm">
                Some tracks need another metadata refresh. Imported tracks
                remain in your library.
              </p>
              <Button
                type="button"
                variant="outline"
                aria-label="Retry failed metadata"
                class="mt-2"
                :disabled="isRetryingMetadata"
                @click="emit('retryMetadata')"
                >{{
                  isRetryingMetadata ? "Retrying…" : "Retry failed metadata"
                }}</Button
              >
            </template>
          </section>

          <form
            class="grid gap-4"
            aria-label="Import music from YouTube"
            @submit.prevent="submitYouTubeUrls"
          >
            <section class="window-panel p-5">
              <div class="flex items-start gap-4">
                <span
                  class="grid size-10 shrink-0 place-items-center rounded-xl bg-(--accent-soft) text-accent [&>svg]:size-5"
                  aria-hidden="true"
                >
                  <ArrowDownToLine />
                </span>
                <div>
                  <h2 class="text-base font-semibold">Import from YouTube</h2>
                  <p class="mt-1 text-[0.78rem] leading-5 text-(--muted-text)">
                    Paste one link per line. Imports run in the background and
                    do not interrupt playback.
                  </p>
                </div>
              </div>

              <label
                class="mt-5 block text-[0.7rem] font-semibold tracking-[0.04em] text-(--muted-text) uppercase"
                for="youtube-import-urls"
              >
                YouTube URLs
              </label>
              <textarea
                id="youtube-import-urls"
                v-model="youtubeUrls"
                aria-label="YouTube URLs"
                class="mt-2 min-h-40 w-full resize-y rounded-lg border border-(--line-strong) bg-(--glass-control) px-3.5 py-3 font-mono text-[0.76rem] leading-5 text-(--text) outline-none transition-colors placeholder:text-(--subtle-text) focus:border-(--focus-ring)"
                placeholder="Paste one URL per line&#10;https://youtube.com/watch?v=…&#10;https://youtube.com/playlist?list=…&#10;https://youtube.com/@artist/videos"
                :disabled="isImporting || Boolean(activeProgress)"
                spellcheck="false"
              />

              <div class="mt-3 flex items-center justify-between gap-4">
                <p class="text-[0.72rem] text-(--muted-text)">
                  {{ importSources.length }}
                  {{ importSources.length === 1 ? "source" : "sources" }} ready
                </p>
                <Button
                  type="submit"
                  :disabled="
                    isImporting ||
                    Boolean(activeProgress) ||
                    importSources.length === 0
                  "
                >
                  <ArrowDownToLine aria-hidden="true" />
                  {{ isImporting ? "Importing…" : "Import to library" }}
                </Button>
              </div>
            </section>

            <section
              class="window-panel-muted p-4"
              aria-label="Import progress"
            >
              <div class="flex items-center justify-between gap-4">
                <div class="flex items-center gap-2 text-[0.76rem] font-medium">
                  <TerminalSquare
                    class="size-4 text-accent"
                    aria-hidden="true"
                  />
                  Import terminal
                </div>
                <span class="text-[0.7rem] text-(--muted-text)">
                  {{ progress?.completedSources ?? 0 }} /
                  {{ progress?.totalSources ?? 0 }} sources
                </span>
              </div>
              <p class="mt-2 text-[0.7rem] text-(--muted-text)">
                {{ progress?.importedTracks ?? 0 }} track(s) found
                <template v-if="progress?.skippedMemberOnly">
                  · {{ progress.skippedMemberOnly }} members-only track(s)
                  skipped
                </template>
              </p>
              <p
                v-if="progress?.phase === 'cancelled'"
                role="status"
                class="mt-2 text-sm"
              >
                Import cancelled. {{ progress.importedTracks }} track(s) remain
                in your library.
              </p>
              <Button
                v-if="activeProgress && progress"
                type="button"
                variant="outline"
                aria-label="Cancel import"
                class="mt-3"
                :disabled="isCancelling"
                @click="emit('cancelImport', progress.runId)"
                >{{ isCancelling ? "Cancelling…" : "Cancel import" }}</Button
              >
              <progress
                class="mt-3 h-1.5 w-full overflow-hidden rounded-full accent-accent"
                :value="progressValue"
                max="100"
              >
                {{ progressPercent }}%
              </progress>
              <ScrollArea class="window-panel-muted mt-3 max-h-36 min-h-24">
                <output
                  class="block min-h-24 break-words p-3 font-mono text-[0.69rem] leading-5 text-(--muted-text)"
                  role="log"
                  aria-live="polite"
                >
                  <span
                    v-for="(log, index) in logs"
                    :key="`${index}-${log}`"
                    class="block"
                    >&gt; {{ log }}</span
                  >
                </output>
              </ScrollArea>
            </section>
          </form>
        </div>
      </ScrollArea>
    </section>
  </main>
</template>

<style scoped>
progress::-webkit-progress-bar {
  background: var(--surface-muted);
}

progress::-webkit-progress-value {
  background: var(--accent);
}

progress::-moz-progress-bar {
  background: var(--accent);
}
</style>
