<script setup lang="ts">
import { computed } from 'vue'

import type { FollowedArtist, FollowUpload } from '@/api'
import { ScrollArea } from '@/components/ui/scroll-area'
import { formatDuration } from '@/lib/time'

const props = defineProps<{
  artists: FollowedArtist[]
  libraryTrackIds: string[]
  pendingChannelIds: string[]
  isChecking: boolean
  isImporting?: boolean
  isLoading?: boolean
  loadFailed?: boolean
  errorMessage?: string
}>()

const emit = defineEmits<{
  addToLibrary: [urls: string[]]
  check: []
  checkArtist: [channelId: string]
  retryLoad: []
  unfollow: [channelId: string]
}>()

const timestampFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'medium',
  timeStyle: 'short',
})
const inLibrary = computed(() => new Set(props.libraryTrackIds))

function isPending(artist: FollowedArtist): boolean {
  return props.pendingChannelIds.includes(artist.channelId)
}

function status(artist: FollowedArtist): string {
  if (artist.checking) return 'Checking for new uploads…'
  if (artist.lastCheckedAtMs === null) return 'Not checked yet'
  return `Last checked ${timestampFormat.format(new Date(artist.lastCheckedAtMs))}`
}

function uploadLabel(upload: FollowUpload): string {
  return upload.isNew ? 'New since you followed' : 'Recent upload'
}
</script>

<template>
  <section class="min-h-0" aria-label="Following" data-library-following>
    <ScrollArea class="size-full">
      <div class="grid gap-5 px-5 pb-6">
        <div class="flex items-center justify-between gap-3">
          <p class="text-[0.78rem] text-(--muted-text)">
            New uploads are checked on startup and every hour. Nothing is added to your library
            until you choose it.
          </p>
          <button
            class="shrink-0 cursor-pointer rounded-md border border-(--line) bg-transparent px-3 py-1.5 text-[0.78rem] text-(--text) hover:bg-[oklch(0.72_0.025_258/0.1)] focus-visible:ring-2 focus-visible:ring-(--focus-ring) disabled:cursor-default disabled:opacity-40"
            data-check-following
            :disabled="props.isChecking || props.artists.length === 0"
            type="button"
            @click="emit('check')"
          >
            {{ props.isChecking ? 'Checking…' : 'Check for new uploads' }}
          </button>
        </div>

        <p v-if="props.errorMessage" class="text-[0.78rem] text-red-300" role="alert">
          {{ props.errorMessage }}
        </p>

        <div v-if="props.loadFailed" class="flex items-center justify-between gap-3">
          <p class="text-[0.82rem] text-(--muted-text)">Followed artists could not be loaded.</p>
          <button
            class="shrink-0 cursor-pointer rounded-md border border-(--line) bg-transparent px-3 py-1.5 text-[0.78rem] text-(--text) hover:bg-[oklch(0.72_0.025_258/0.1)] focus-visible:ring-2 focus-visible:ring-(--focus-ring) disabled:cursor-default disabled:opacity-40"
            data-retry-following
            :disabled="props.isLoading"
            type="button"
            @click="emit('retryLoad')"
          >
            {{ props.isLoading ? 'Loading…' : 'Retry loading' }}
          </button>
        </div>
        <p v-else-if="props.artists.length === 0" class="text-[0.82rem] text-(--muted-text)">
          You are not following any artists. Follow an artist from the Artists view.
        </p>

        <section
          v-for="artist in props.artists"
          :key="artist.channelId"
          class="grid gap-2"
          :aria-label="`Following ${artist.name}`"
          data-followed-artist
        >
          <div class="flex items-center justify-between gap-3">
            <div class="min-w-0">
              <h2
                class="overflow-hidden text-[0.88rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
              >
                {{ artist.name }}
              </h2>
              <p class="text-[0.69rem] text-(--muted-text)" aria-live="polite">
                {{ status(artist) }}
              </p>
            </div>
            <div class="flex shrink-0 gap-2">
              <button
                class="cursor-pointer rounded-md border border-(--line) bg-transparent px-2.5 py-1 text-[0.72rem] text-(--text) hover:bg-[oklch(0.72_0.025_258/0.1)] focus-visible:ring-2 focus-visible:ring-(--focus-ring) disabled:cursor-default disabled:opacity-40"
                :aria-label="`Check ${artist.name} for new uploads`"
                :disabled="props.isChecking || artist.checking"
                type="button"
                @click="emit('checkArtist', artist.channelId)"
              >
                Check
              </button>
              <button
                class="cursor-pointer rounded-md border border-(--line) bg-transparent px-2.5 py-1 text-[0.72rem] text-(--text) hover:bg-[oklch(0.72_0.025_258/0.1)] focus-visible:ring-2 focus-visible:ring-(--focus-ring) disabled:cursor-default disabled:opacity-40"
                :aria-label="`Unfollow ${artist.name}`"
                :disabled="isPending(artist)"
                type="button"
                @click="emit('unfollow', artist.channelId)"
              >
                Unfollow
              </button>
            </div>
          </div>

          <p v-if="artist.lastError" class="text-[0.72rem] text-red-300" role="alert">
            The last check failed: {{ artist.lastError }} Showing earlier results.
          </p>
          <p v-if="!artist.baselineEstablished" class="text-[0.72rem] text-(--muted-text)">
            The first check records the uploads that already exist. They appear as recent uploads,
            not new ones.
          </p>
          <p v-else-if="artist.uploads.length === 0" class="text-[0.72rem] text-(--muted-text)">
            No uploads found yet.
          </p>

          <ul v-if="artist.uploads.length" class="grid gap-0.5">
            <li
              v-for="upload in artist.uploads"
              :key="upload.videoId"
              class="flex items-center justify-between gap-3 rounded-md px-2 py-1.5 hover:bg-[oklch(0.72_0.025_258/0.08)]"
              :data-upload="upload.videoId"
            >
              <div class="min-w-0">
                <p
                  class="overflow-hidden text-[0.82rem] text-ellipsis whitespace-nowrap text-(--text)"
                >
                  {{ upload.title }}
                </p>
                <p class="text-[0.69rem] text-(--muted-text)">
                  {{ uploadLabel(upload) }}
                  <template v-if="upload.durationMs !== null">
                    · {{ formatDuration(upload.durationMs) }}
                  </template>
                </p>
              </div>
              <span
                v-if="inLibrary.has(upload.videoId)"
                class="shrink-0 text-[0.72rem] text-(--muted-text)"
              >
                In library
              </span>
              <button
                v-else
                class="shrink-0 cursor-pointer rounded-md border border-(--line) bg-transparent px-2.5 py-1 text-[0.72rem] text-(--text) hover:bg-[oklch(0.72_0.025_258/0.1)] focus-visible:ring-2 focus-visible:ring-(--focus-ring) disabled:cursor-default disabled:opacity-40"
                :aria-label="`Add ${upload.title} to library`"
                :disabled="props.isImporting"
                type="button"
                @click="emit('addToLibrary', [upload.url])"
              >
                Add to library
              </button>
            </li>
          </ul>
        </section>
      </div>
    </ScrollArea>
  </section>
</template>
