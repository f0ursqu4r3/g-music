<script setup lang="ts">
import { ArrowDownToLine, LoaderCircle, Search } from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

import {
  playbackApi,
  type ImportProgress,
  type MediaItem,
  type MetadataRefreshSnapshot,
} from '@/api'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { formatDuration } from '@/lib/time'

interface Props {
  isImporting: boolean
  errorMessage?: string
  progress?: ImportProgress | null
  isCancelling?: boolean
  metadataRefreshes?: MetadataRefreshSnapshot
  isRetryingMetadata?: boolean
}

const props = defineProps<Props>()

const emit = defineEmits<{
  importYoutubeUrls: [urls: string[]]
  cancelImport: [runId: number]
  retry: []
  retryMetadata: []
}>()

const youtubeUrls = ref('')
const searchQuery = ref('')
const searchResults = ref<MediaItem[]>([])
const searchError = ref('')
const isSearching = ref(false)
const hasSearched = ref(false)
let searchGeneration = 0
let submittedUrls = ''
let lastImportSources: string[] = []
const activeProgress = computed(
  () => props.progress && !['completed', 'failed', 'cancelled'].includes(props.progress.phase),
)
const importBusy = computed(() => props.isImporting || Boolean(activeProgress.value))
const statusLabel = computed(() => {
  if (props.isCancelling && activeProgress.value) return 'Cancelling import'
  const phase = props.progress?.phase
  if (phase)
    return {
      started: 'Starting import',
      resolving: 'Discovering tracks',
      merging: 'Saving to library',
      completed: 'Import complete',
      failed: 'Import failed',
      cancelled: 'Import cancelled',
    }[phase]
  return props.isImporting ? 'Starting import' : 'Ready to import'
})
const logViewport = ref<HTMLElement | null>(null)

watch(searchQuery, () => {
  searchGeneration += 1
  isSearching.value = false
  hasSearched.value = false
  searchResults.value = []
  searchError.value = ''
})
onBeforeUnmount(() => {
  searchGeneration += 1
})

async function searchYouTube(): Promise<void> {
  const query = searchQuery.value.trim()
  if (!query) return
  const generation = ++searchGeneration
  isSearching.value = true
  searchError.value = ''
  searchResults.value = []
  try {
    const results = await playbackApi.searchYouTube(query)
    if (generation !== searchGeneration) return
    searchResults.value = results.slice(0, 20)
    hasSearched.value = true
  } catch (error) {
    if (generation !== searchGeneration) return
    searchError.value =
      typeof error === 'object' && error !== null && 'message' in error
        ? String(error.message)
        : 'Search failed. Check your connection and retry.'
  } finally {
    if (generation === searchGeneration) isSearching.value = false
  }
}

function importSearchResult(item: MediaItem): void {
  if (props.isImporting || activeProgress.value) return
  submittedUrls = ''
  lastImportSources = [
    item.sourceUrl || `https://www.youtube.com/watch?v=${encodeURIComponent(item.id)}`,
  ]
  emit('importYoutubeUrls', lastImportSources)
}

function retryImport(): void {
  if (props.isImporting || activeProgress.value) return
  if (lastImportSources.length) emit('importYoutubeUrls', lastImportSources)
  else emit('retry')
}
const logs = ref<string[]>([])
let currentRunId: number | undefined
const importSources = computed(() => [
  ...new Set(
    youtubeUrls.value
      .split(/\r?\n/)
      .map((url) => url.trim())
      .filter(Boolean),
  ),
])
const progressPercent = computed(() => {
  const total = props.progress?.totalSources ?? 0
  if (total === 0) {
    return 0
  }

  return Math.min(
    100,
    Math.max(0, Math.round(((props.progress?.completedSources ?? 0) / total) * 100)),
  )
})
const progressValue = computed<number | undefined>(() => {
  const progress = props.progress
  if (
    !progress ||
    (activeProgress.value &&
      (progress.phase !== 'resolving' || progress.completedSources === 0 || !progress.totalSources))
  ) {
    return undefined
  }

  return progressPercent.value
})

watch(
  () => props.progress,
  (progress) => {
    if (!progress) {
      return
    }

    const newRun = currentRunId !== progress.runId
    const viewport = logViewport.value
    const followLog =
      newRun || !viewport || viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 24
    if (newRun) {
      currentRunId = progress.runId
      logs.value = []
    }

    if (progress.phase === 'completed' && submittedUrls && youtubeUrls.value === submittedUrls) {
      youtubeUrls.value = ''
      submittedUrls = ''
    }

    const entry = progress.message.trim()
    if (entry && logs.value[logs.value.length - 1] !== entry) {
      logs.value.push(entry)
      logs.value = logs.value.slice(-300)
      if (followLog)
        void nextTick(() => {
          if (logViewport.value) logViewport.value.scrollTop = logViewport.value.scrollHeight
        })
    }
  },
  { immediate: true },
)

function submitYouTubeUrls(): void {
  if (props.isImporting || activeProgress.value || importSources.value.length === 0) {
    return
  }

  submittedUrls = youtubeUrls.value
  lastImportSources = importSources.value
  emit('importYoutubeUrls', importSources.value)
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

    <section class="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)_auto] pt-7">
      <header class="flex items-center justify-between gap-4 border-b border-(--line) px-6 py-4">
        <h1 class="window-title">Import Music</h1>
      </header>

      <ScrollArea class="min-h-0 size-full" type="scroll">
        <div class="px-6 py-5">
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

          <form aria-label="Import music from YouTube" @submit.prevent="submitYouTubeUrls">
            <label for="youtube-import-urls" class="block text-sm font-semibold"
              >YouTube URLs</label
            >
            <p id="youtube-import-help" class="mt-1 text-xs leading-5 text-(--muted-text)">
              Videos, playlists, albums, or channels. One link per line.
            </p>
            <textarea
              id="youtube-import-urls"
              v-model="youtubeUrls"
              aria-label="YouTube URLs"
              aria-describedby="youtube-import-help"
              rows="3"
              class="mt-3 block w-full resize-none rounded-lg border border-(--line-strong) bg-(--glass-control) px-3 py-2.5 font-mono text-xs leading-5 text-(--text) outline-none transition-colors placeholder:text-(--subtle-text) focus:border-(--focus-ring)"
              placeholder="Paste one URL per line&#10;https://youtube.com/watch?v=…"
              :disabled="importBusy"
              spellcheck="false"
            />
            <div class="mt-3 flex items-center justify-between gap-4">
              <p class="text-xs text-(--muted-text)">
                <template v-if="importBusy">Import in progress</template>
                <template v-else-if="importSources.length"
                  >{{ importSources.length }}
                  {{ importSources.length === 1 ? 'source' : 'sources' }}
                  ready</template
                >
                <template v-else>Paste a link to get started</template>
              </p>
              <Button type="submit" size="sm" :disabled="importBusy || !importSources.length">
                <ArrowDownToLine aria-hidden="true" />
                Import to library
              </Button>
            </div>
          </form>

          <section
            class="mt-5 border-t border-(--line) pt-4"
            aria-labelledby="youtube-search-heading"
          >
            <h2 id="youtube-search-heading" class="text-sm font-semibold">Search YouTube</h2>

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
                class="h-9 min-w-0 flex-1 rounded-lg border border-(--line-strong) bg-(--glass-control) px-3 text-sm outline-none focus:border-(--focus-ring)"
              />
              <Button
                type="submit"
                size="sm"
                variant="outline"
                :disabled="!searchQuery.trim() || isSearching"
                ><Search class="size-3.5" aria-hidden="true" />{{
                  isSearching ? 'Searching…' : 'Search'
                }}</Button
              >
            </form>
            <p v-if="isSearching" role="status" class="mt-3 text-sm text-(--muted-text)">
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
            <p v-else-if="hasSearched" role="status" class="mt-3 text-xs text-(--muted-text)">
              {{ searchResults.length }} results · Up to 20 shown
            </p>
            <ol
              v-if="searchResults.length"
              class="mt-2 divide-y divide-(--line)"
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
                    {{ item.artist }}<template v-if="item.album"> · {{ item.album }}</template> ·
                    {{ formatDuration(item.durationMs) }}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  :aria-label="`Import ${item.title}`"
                  size="sm"
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
            class="mt-5 border-t border-(--line) pt-4"
          >
            <h2 class="text-sm font-semibold">Metadata refresh</h2>
            <p class="mt-1 text-xs text-(--muted-text)">
              {{ metadataRefreshes.completedTracks }} of
              {{ metadataRefreshes.totalTracks }} refreshed
            </p>
            <template v-if="metadataRefreshes.jobs.some((job) => job.state === 'failed')">
              <p class="mt-2 text-sm">
                Some tracks need another metadata refresh. Imported tracks remain in your library.
              </p>
              <Button
                type="button"
                variant="outline"
                aria-label="Retry failed metadata"
                class="mt-2"
                :disabled="isRetryingMetadata"
                @click="emit('retryMetadata')"
                >{{ isRetryingMetadata ? 'Retrying…' : 'Retry failed metadata' }}</Button
              >
            </template>
          </section>
        </div>
      </ScrollArea>
      <footer class="border-t border-(--line) px-6 py-4">
        <section v-if="progress || isImporting" aria-label="Import progress">
          <div class="flex items-center justify-between gap-4">
            <div class="min-w-0">
              <h2
                data-import-status
                role="status"
                class="flex items-center gap-2 text-sm font-semibold"
              >
                <LoaderCircle
                  v-if="importBusy"
                  class="size-3.5 shrink-0 animate-spin text-accent motion-reduce:animate-none"
                  aria-hidden="true"
                />
                {{ statusLabel }}
              </h2>
              <p class="mt-1 text-xs text-(--muted-text)">
                {{ progress?.importedTracks ?? 0 }}
                {{ progress?.importedTracks === 1 ? 'track' : 'tracks' }} found
                <template v-if="progress?.totalSources">
                  · {{ progress.completedSources }} /
                  {{ progress.totalSources }}
                  {{ progress.totalSources === 1 ? 'source' : 'sources' }}</template
                >
                <template v-if="progress?.skippedMemberOnly">
                  · {{ progress.skippedMemberOnly }} members-only
                  {{ progress.skippedMemberOnly === 1 ? 'track' : 'tracks' }}
                  skipped</template
                >
              </p>
            </div>
            <Button
              v-if="activeProgress && progress"
              type="button"
              variant="outline"
              size="sm"
              aria-label="Cancel import"
              class="shrink-0"
              :disabled="isCancelling"
              @click="emit('cancelImport', progress.runId)"
              >{{ isCancelling ? 'Cancelling…' : 'Cancel import' }}</Button
            >
          </div>
          <p v-if="progress?.phase === 'cancelled'" role="status" class="mt-2 text-sm">
            {{ progress.importedTracks }}
            {{ progress.importedTracks === 1 ? 'track remains' : 'tracks remain' }}
            in your library.
          </p>
          <progress
            v-if="importBusy || progress?.phase === 'completed'"
            aria-label="Import progress"
            class="mt-3 block h-1 w-full overflow-hidden rounded-full accent-accent"
            :value="progressValue"
            max="100"
          >
            {{ progressPercent }}%
          </progress>
          <ScrollArea
            v-if="logs.length"
            data-import-log
            class="mt-3 h-24 rounded-md bg-(--glass-control) [@media(max-height:480px)]:h-12"
            :viewport-ref="(element) => (logViewport = element)"
          >
            <output
              class="block break-words px-3 py-2 font-mono text-[0.69rem] leading-5 text-(--muted-text) select-text"
              role="log"
              aria-label="Import activity"
              aria-live="polite"
            >
              <span v-for="(log, index) in logs" :key="`${index}-${log}`" class="block">{{
                log
              }}</span>
            </output>
          </ScrollArea>
        </section>
        <p v-else class="text-xs leading-5 text-(--muted-text)">
          Imports run in the background without interrupting playback.
        </p>
      </footer>
    </section>
  </main>
</template>

<style scoped>
progress {
  appearance: none;
  border: 0;
  background: var(--surface-muted);
}

progress:indeterminate {
  background: var(--accent-soft);
}

progress:indeterminate::-webkit-progress-bar {
  background: var(--accent-soft);
}

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
