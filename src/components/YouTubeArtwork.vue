<script setup lang="ts">
import { computed, ref, watch, type Component } from 'vue'

import { artworkApi } from '@/api'

const props = defineProps<{
  videoId: string | null | undefined
  missingIcon?: Component | null
}>()

const source = ref<string | null>(null)
let requestId = 0
const youtubeVideoId = computed(() => {
  const videoId = props.videoId?.trim() ?? ''
  return /^[A-Za-z0-9_-]{11}$/.test(videoId) ? videoId : null
})
watch(
  youtubeVideoId,
  async (videoId) => {
    const currentRequest = ++requestId
    source.value = null
    if (!videoId) {
      return
    }
    try {
      const resolved = await artworkApi.resolveYouTube(videoId)
      if (currentRequest === requestId) {
        source.value = resolved
      }
    } catch {
      if (currentRequest === requestId) {
        source.value = null
      }
    }
  },
  { immediate: true },
)

function clearBrokenImage(): void {
  source.value = null
}
</script>

<template>
  <div class="youtube-artwork" aria-hidden="true">
    <img
      v-if="source"
      :src="source"
      alt=""
      decoding="async"
      draggable="false"
      referrerpolicy="no-referrer"
      @error="clearBrokenImage"
    />
    <div
      v-else
      class="artwork-placeholder flex inset-0 justify-center items-center"
      data-artwork-placeholder
      aria-hidden="true"
    >
      <component v-if="missingIcon" :is="missingIcon" aria-hidden="true" />
    </div>
  </div>
</template>

<style scoped>
.youtube-artwork {
  position: relative;
  display: block;
  overflow: hidden;
  background: var(--artwork-c);
}

.artwork-placeholder {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(
      circle at 18% 12%,
      color-mix(in oklch, var(--artwork-a), transparent 16%),
      transparent 42%
    ),
    linear-gradient(145deg, var(--artwork-a), var(--artwork-b) 54%, var(--artwork-c));
}

.youtube-artwork > img {
  position: absolute;
  inset: 0;
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}
</style>
