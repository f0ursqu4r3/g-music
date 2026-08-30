<script setup lang="ts">
import { Mic2 } from "lucide-vue-next";

import type { MediaItem } from "@/api";
import { formatDuration } from "@/lib/time";
import type { LibraryAlbum, LibraryArtist } from "./types";
import YouTubeArtwork from "../YouTubeArtwork.vue";

const props = defineProps<{
  selectedTrack: MediaItem | null;
  selectedAlbum: LibraryAlbum | null;
  selectedArtist: LibraryArtist | null;
}>();
</script>

<template>
  <aside
    class="library-info-panel col-start-3 row-start-1 min-h-0 overflow-y-auto border-l border-(--line) p-5 max-[1040px]:hidden"
    :data-library-info="
      props.selectedTrack
        ? 'track'
        : props.selectedAlbum
          ? 'album'
          : props.selectedArtist
            ? 'artist'
            : 'empty'
    "
    aria-label="Selected library item details"
  >
    <div v-if="props.selectedTrack" data-library-info="track">
      <div class="cover-art mb-5 aspect-square w-full rounded-xl">
        <YouTubeArtwork
          class="absolute inset-0 size-full object-cover"
          :video-id="props.selectedTrack.id"
        />
      </div>
      <p
        class="mb-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
      >
        Track
      </p>
      <h2 class="m-0 text-lg font-semibold text-(--text)">
        {{ props.selectedTrack.title }}
      </h2>
      <p class="mt-1 text-sm text-(--muted-text)">
        {{ props.selectedTrack.artist }}
      </p>
      <dl class="mt-6 grid gap-3 border-t border-(--line) pt-4 text-xs">
        <div class="flex items-start justify-between gap-3">
          <dt class="text-(--muted-text)">Album</dt>
          <dd class="m-0 text-right text-(--text)">
            {{ props.selectedTrack.album || "—" }}
          </dd>
        </div>
        <div class="flex items-start justify-between gap-3">
          <dt class="text-(--muted-text)">Duration</dt>
          <dd class="m-0 text-right text-(--text)">
            {{ formatDuration(props.selectedTrack.durationMs) }}
          </dd>
        </div>
      </dl>
    </div>

    <div v-else-if="props.selectedAlbum" data-library-info="album">
      <div class="cover-art mb-5 aspect-square w-full rounded-xl">
        <YouTubeArtwork
          class="absolute inset-0 size-full object-cover"
          :video-id="props.selectedAlbum.videoId"
        />
      </div>
      <p
        class="mb-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
      >
        Album
      </p>
      <h2 class="m-0 text-lg font-semibold text-(--text)">
        {{ props.selectedAlbum.title }}
      </h2>
      <p class="mt-1 text-sm text-(--muted-text)">
        {{ props.selectedAlbum.artist }}
      </p>
      <dl class="mt-6 grid gap-3 border-t border-(--line) pt-4 text-xs">
        <div class="flex items-start justify-between gap-3">
          <dt class="text-(--muted-text)">Tracks</dt>
          <dd class="m-0 text-right text-(--text)">
            {{ props.selectedAlbum.trackCount }}
          </dd>
        </div>
        <div class="flex items-start justify-between gap-3">
          <dt class="text-(--muted-text)">Duration</dt>
          <dd class="m-0 text-right text-(--text)">
            {{ formatDuration(props.selectedAlbum.durationMs) }}
          </dd>
        </div>
      </dl>
    </div>

    <div v-else-if="props.selectedArtist" data-library-info="artist">
      <div
        class="cover-art mb-5 grid aspect-square w-full place-items-center rounded-full [&>svg]:size-20 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
      >
        <Mic2 aria-hidden="true" />
        <YouTubeArtwork
          class="absolute inset-0 size-full object-cover"
          :video-id="props.selectedArtist.videoId"
        />
      </div>
      <p
        class="mb-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
      >
        Artist
      </p>
      <h2 class="m-0 text-lg font-semibold text-(--text)">
        {{ props.selectedArtist.name }}
      </h2>
      <dl class="mt-6 grid gap-3 border-t border-(--line) pt-4 text-xs">
        <div class="flex items-start justify-between gap-3">
          <dt class="text-(--muted-text)">Albums</dt>
          <dd class="m-0 text-right text-(--text)">
            {{ props.selectedArtist.albumCount }}
          </dd>
        </div>
        <div class="flex items-start justify-between gap-3">
          <dt class="text-(--muted-text)">Tracks</dt>
          <dd class="m-0 text-right text-(--text)">
            {{ props.selectedArtist.trackCount }}
          </dd>
        </div>
        <div class="flex items-start justify-between gap-3">
          <dt class="text-(--muted-text)">Duration</dt>
          <dd class="m-0 text-right text-(--text)">
            {{ formatDuration(props.selectedArtist.durationMs) }}
          </dd>
        </div>
      </dl>
    </div>

    <div
      v-else
      class="grid min-h-full place-items-center text-center text-sm text-(--muted-text)"
    >
      Select a track, album, or artist to see its metadata.
    </div>
  </aside>
</template>

<style scoped>
.cover-art {
  position: relative;
  overflow: hidden;
  background: linear-gradient(
    138deg,
    var(--artwork-a),
    var(--artwork-b) 58%,
    var(--artwork-c)
  );
}
</style>
