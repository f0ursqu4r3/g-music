<script setup lang="ts">
import { ArrowDownToLine, TerminalSquare } from "lucide-vue-next";
import { computed, ref, watch } from "vue";

import type { ImportProgress } from "@/api";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";

interface Props {
  isImporting: boolean;
  errorMessage?: string;
  progress?: ImportProgress | null;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  importYoutubeUrls: [urls: string[]];
}>();

const youtubeUrls = ref("");
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

    const entry = progress.message.trim();
    if (entry && logs.value[logs.value.length - 1] !== entry) {
      logs.value.push(entry);
    }
  },
  { immediate: true },
);

function submitYouTubeUrls(): void {
  if (props.isImporting || importSources.value.length === 0) {
    return;
  }

  emit("importYoutubeUrls", importSources.value);
  youtubeUrls.value = "";
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
            class="window-alert-danger mb-4 px-3 py-2 text-sm"
            role="alert"
          >
            {{ errorMessage }}
          </p>

          <form
            class="grid gap-4"
            aria-label="Import music from YouTube"
            @submit.prevent="submitYouTubeUrls"
          >
            <section class="window-panel p-5">
              <div class="flex items-start gap-4">
                <span
                  class="grid size-10 shrink-0 place-items-center rounded-xl bg-[oklch(0.72_0.08_288/0.14)] text-accent [&>svg]:size-5"
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
                :disabled="isImporting"
                spellcheck="false"
              />

              <div class="mt-3 flex items-center justify-between gap-4">
                <p class="text-[0.72rem] text-(--muted-text)">
                  {{ importSources.length }}
                  {{ importSources.length === 1 ? "source" : "sources" }} ready
                </p>
                <Button
                  type="submit"
                  :disabled="isImporting || importSources.length === 0"
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
              <progress
                class="mt-3 h-1.5 w-full overflow-hidden rounded-full accent-accent"
                :value="progressValue"
                max="100"
              >
                {{ progressPercent }}%
              </progress>
              <ScrollArea class="window-panel-muted mt-3 max-h-36 min-h-24">
                <output
                  class="block min-h-24 p-3 font-mono text-[0.69rem] leading-5 text-[oklch(0.82_0.025_258)]"
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
