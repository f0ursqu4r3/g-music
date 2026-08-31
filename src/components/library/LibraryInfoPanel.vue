<script setup lang="ts">
import { ExternalLink, Mic2 } from "lucide-vue-next";
import { computed, ref, watch } from "vue";

import type { MediaItem } from "@/api";
import { formatDuration } from "@/lib/time";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { LibraryAlbum, LibraryArtist } from "./types";
import YouTubeArtwork from "../YouTubeArtwork.vue";

const props = defineProps<{
  isOpen: boolean;
  selectedTrack: MediaItem | null;
  selectedAlbum: LibraryAlbum | null;
  selectedArtist: LibraryArtist | null;
}>();

interface TrackDetail {
  label: string;
  value: string;
}

const numberFormat = new Intl.NumberFormat();
const timestampFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});
const isDescriptionExpanded = ref(false);
const hasLongDescription = computed(
  () => (props.selectedTrack?.description?.length ?? 0) > 280,
);
const trackDetails = computed<TrackDetail[]>(() => {
  const track = props.selectedTrack;
  if (!track) {
    return [];
  }

  const optionalDetails: Array<TrackDetail | null> = [
    track.albumArtist
      ? { label: "Album artist", value: track.albumArtist }
      : null,
    track.trackNumber !== undefined && track.trackNumber !== null
      ? { label: "Track number", value: String(track.trackNumber) }
      : null,
    track.discNumber !== undefined && track.discNumber !== null
      ? { label: "Disc number", value: String(track.discNumber) }
      : null,
    track.releaseDate ? { label: "Released", value: track.releaseDate } : null,
    track.uploadDate ? { label: "Uploaded", value: track.uploadDate } : null,
    track.channel ? { label: "Channel", value: track.channel } : null,
    track.channelId ? { label: "Channel ID", value: track.channelId } : null,
    track.uploader ? { label: "Uploader", value: track.uploader } : null,
    track.uploaderId ? { label: "Uploader ID", value: track.uploaderId } : null,
    track.label ? { label: "Label", value: track.label } : null,
    track.genres?.length
      ? { label: "Genres", value: track.genres.join(", ") }
      : null,
    track.categories?.length
      ? { label: "Categories", value: track.categories.join(", ") }
      : null,
    track.tags?.length ? { label: "Tags", value: track.tags.join(", ") } : null,
    track.language ? { label: "Language", value: track.language } : null,
    track.availability
      ? { label: "Availability", value: track.availability }
      : null,
    { label: "Stream", value: track.isLive ? "Live" : "On demand" },
    track.viewCount !== undefined && track.viewCount !== null
      ? { label: "Views", value: numberFormat.format(track.viewCount) }
      : null,
    track.likeCount !== undefined && track.likeCount !== null
      ? { label: "Likes", value: numberFormat.format(track.likeCount) }
      : null,
    track.provider ? { label: "Provider", value: track.provider } : null,
    { label: "Track ID", value: track.id },
    {
      label: "Metadata",
      value: track.metadataDirty ? "Refresh pending" : "Complete",
    },
    {
      label: "Play count",
      value: numberFormat.format(track.playCount ?? 0),
    },
    track.lastPlayedAtMs
      ? {
          label: "Last played",
          value: timestampFormat.format(new Date(track.lastPlayedAtMs)),
        }
      : null,
  ];

  return [
    { label: "Album", value: track.album || "—" },
    { label: "Duration", value: formatDuration(track.durationMs) },
    ...optionalDetails.filter(
      (detail): detail is TrackDetail => detail !== null,
    ),
  ];
});

function formatPlayHistory(timestampMs: number): string {
  return timestampFormat.format(new Date(timestampMs));
}

watch(
  () => props.selectedTrack?.description,
  () => {
    isDescriptionExpanded.value = false;
  },
);
</script>

<template>
  <aside
    class="library-info-panel col-start-3 row-start-1 min-h-0 w-68 min-w-68 overflow-hidden border-l border-(--line) transition-all duration-200 ease-out motion-reduce:transition-none max-[1040px]:hidden"
    :class="
      props.isOpen
        ? 'translate-x-0 opacity-100'
        : 'translate-x-3 pointer-events-none opacity-0'
    "
    :data-library-info="
      props.selectedTrack
        ? 'track'
        : props.selectedAlbum
          ? 'album'
          : props.selectedArtist
            ? 'artist'
            : 'empty'
    "
    data-library-selected-sidebar
    :aria-hidden="props.isOpen ? undefined : 'true'"
    aria-label="Selected library item details"
    :inert="props.isOpen ? undefined : true"
  >
    <ScrollArea class="size-full">
      <div class="p-5">
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
          <section
            v-if="props.selectedTrack.description"
            class="mt-5 border-t border-(--line) pt-4"
            aria-label="Description"
          >
            <h3
              class="m-0 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
            >
              Description
            </h3>
            <p
              id="track-description"
              class="mt-2 text-sm leading-5 wrap-break-word text-(--text)"
              :class="{
                'line-clamp-6': hasLongDescription && !isDescriptionExpanded,
              }"
              data-track-description
            >
              {{ props.selectedTrack.description }}
            </p>
            <button
              v-if="hasLongDescription"
              class="mt-2 cursor-pointer border-0 bg-transparent p-0 text-xs font-medium text-accent hover:underline focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
              :aria-expanded="isDescriptionExpanded"
              aria-controls="track-description"
              data-track-description-toggle
              type="button"
              @click="isDescriptionExpanded = !isDescriptionExpanded"
            >
              {{ isDescriptionExpanded ? "Show less" : "Show more" }}
            </button>
          </section>
          <dl class="mt-6 grid gap-3 border-t border-(--line) pt-4 text-xs">
            <div
              v-for="detail in trackDetails"
              :key="detail.label"
              class="flex items-start justify-between gap-3"
            >
              <dt class="shrink-0 text-(--muted-text)">{{ detail.label }}</dt>
              <dd class="m-0 break-all text-right text-(--text)">
                {{ detail.value }}
              </dd>
            </div>
            <div
              v-if="props.selectedTrack.sourceUrl"
              class="flex items-start justify-between gap-3"
            >
              <dt class="shrink-0 text-(--muted-text)">Source</dt>
              <dd class="m-0 min-w-0 text-right">
                <a
                  :href="props.selectedTrack.sourceUrl"
                  class="inline-flex max-w-full items-center gap-1 break-all text-accent underline-offset-2 hover:underline"
                  data-track-source
                  rel="noreferrer"
                  target="_blank"
                >
                  Open source
                  <ExternalLink class="size-3 shrink-0" aria-hidden="true" />
                </a>
              </dd>
            </div>
            <div
              v-if="props.selectedTrack.thumbnailUrl"
              class="flex items-start justify-between gap-3"
            >
              <dt class="shrink-0 text-(--muted-text)">Artwork</dt>
              <dd class="m-0 min-w-0 text-right">
                <a
                  :href="props.selectedTrack.thumbnailUrl"
                  class="inline-flex max-w-full items-center gap-1 break-all text-accent underline-offset-2 hover:underline"
                  data-track-thumbnail
                  rel="noreferrer"
                  target="_blank"
                >
                  Open image
                  <ExternalLink class="size-3 shrink-0" aria-hidden="true" />
                </a>
              </dd>
            </div>
          </dl>
          <section
            v-if="props.selectedTrack.playHistoryMs?.length"
            class="mt-6 border-t border-(--line) pt-4"
            aria-label="Play history"
          >
            <h3
              class="m-0 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
            >
              Play history
            </h3>
            <ol
              class="mt-2 grid gap-1 pl-4 text-xs text-(--text)"
              data-play-history
            >
              <li
                v-for="timestampMs in props.selectedTrack.playHistoryMs"
                :key="timestampMs"
              >
                {{ formatPlayHistory(timestampMs) }}
              </li>
            </ol>
          </section>
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
      </div>
    </ScrollArea>
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
