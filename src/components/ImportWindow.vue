<script setup lang="ts">
import { ArrowDownToLine } from "lucide-vue-next";
import { computed, ref } from "vue";

import { Button } from "@/components/ui/button";

interface Props {
  isUpdating: boolean;
  errorMessage?: string;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  importYoutubeUrls: [urls: string[]];
}>();

const youtubeUrls = ref("");
const importSources = computed(() => [
  ...new Set(
    youtubeUrls.value
      .split(/\r?\n/)
      .map((url) => url.trim())
      .filter(Boolean),
  ),
]);

function submitYouTubeUrls(): void {
  if (props.isUpdating || importSources.value.length === 0) {
    return;
  }

  emit("importYoutubeUrls", importSources.value);
  youtubeUrls.value = "";
}
</script>

<template>
  <main
    class="import-window relative grid min-h-screen content-start gap-6 overflow-auto bg-[radial-gradient(circle_at_12%_0%,oklch(0.48_0.09_274/0.2),transparent_40%),var(--glass-window)] px-8 pt-10 pb-8 text-(--text)"
    aria-label="Import music"
  >
    <header>
      <h1 class="text-2xl font-semibold tracking-[-0.035em]">Import Music</h1>
      <p class="mt-1 text-[0.78rem] text-(--muted-text)">
        Add videos, playlists, albums, channels, or complete artist pages.
      </p>
    </header>

    <p
      v-if="errorMessage"
      class="m-0 rounded-lg border border-red-500/25 bg-red-500/8 px-3 py-2 text-sm text-red-300"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <form
      class="grid gap-6"
      aria-label="Import music from YouTube"
      @submit.prevent="submitYouTubeUrls"
    >
      <section
        class="rounded-xl border border-(--line) bg-[oklch(0.22_0.025_258/0.25)] p-6"
      >
        <div class="flex items-start gap-4">
          <span
            class="grid size-11 shrink-0 place-items-center rounded-xl bg-[oklch(0.72_0.08_288/0.14)] text-accent [&>svg]:size-5"
            aria-hidden="true"
          >
            <ArrowDownToLine />
          </span>
          <div>
            <h2 class="text-base font-semibold">Import from YouTube</h2>
            <p class="mt-1 text-[0.78rem] leading-5 text-(--muted-text)">
              Paste one or more links. Artist and channel pages import every
              playable item that YouTube provides. Importing does not interrupt
              playback.
            </p>
          </div>
        </div>

        <label
          class="mt-6 block text-[0.7rem] font-semibold tracking-[0.04em] text-(--muted-text) uppercase"
          for="youtube-import-urls"
        >
          YouTube URLs
        </label>
        <textarea
          id="youtube-import-urls"
          v-model="youtubeUrls"
          aria-label="YouTube URLs"
          class="mt-2 min-h-52 w-full resize-y rounded-lg border border-(--line-strong) bg-[oklch(0.16_0.02_258/0.42)] px-3.5 py-3 font-mono text-[0.76rem] leading-5 text-(--text) outline-none transition-colors placeholder:text-(--subtle-text) focus:border-(--focus-ring)"
          placeholder="Paste one URL per line\nhttps://youtube.com/watch?v=…\nhttps://youtube.com/playlist?list=…\nhttps://youtube.com/@artist/videos"
          :disabled="isUpdating"
          spellcheck="false"
        />

        <div class="mt-3 flex items-center justify-between gap-4">
          <p class="text-[0.72rem] text-(--muted-text)">
            {{ importSources.length }}
            {{ importSources.length === 1 ? "source" : "sources" }} ready
          </p>
          <Button
            type="submit"
            :disabled="isUpdating || importSources.length === 0"
          >
            <ArrowDownToLine aria-hidden="true" />
            {{ isUpdating ? "Importing…" : "Import to library" }}
          </Button>
        </div>
      </section>

      <section class="grid grid-cols-3 gap-3 max-[620px]:grid-cols-1">
        <div class="rounded-lg border border-(--line) p-4">
          <h2 class="text-[0.78rem] font-semibold">Videos</h2>
          <p class="mt-1 text-[0.7rem] leading-4 text-(--muted-text)">
            Add individual videos or music tracks.
          </p>
        </div>
        <div class="rounded-lg border border-(--line) p-4">
          <h2 class="text-[0.78rem] font-semibold">Playlists and albums</h2>
          <p class="mt-1 text-[0.7rem] leading-4 text-(--muted-text)">
            Expand a collection into ordered library tracks.
          </p>
        </div>
        <div class="rounded-lg border border-(--line) p-4">
          <h2 class="text-[0.78rem] font-semibold">Artists and channels</h2>
          <p class="mt-1 text-[0.7rem] leading-4 text-(--muted-text)">
            Import all playable items from a complete artist page.
          </p>
        </div>
      </section>
    </form>
  </main>
</template>

<style scoped>
.import-window {
  background-color: var(--glass-window);
}
</style>
