<script setup lang="ts">
import { computed, ref, watch } from "vue";

import { artworkApi } from "@/api";

const props = defineProps<{
  videoId: string | null | undefined;
}>();

const source = ref<string | null>(null);
let requestId = 0;
const youtubeVideoId = computed(() => {
  const videoId = props.videoId?.trim() ?? "";
  return /^[A-Za-z0-9_-]{11}$/.test(videoId) ? videoId : null;
});
watch(
  youtubeVideoId,
  async (videoId) => {
    const currentRequest = ++requestId;
    source.value = null;
    if (!videoId) {
      return;
    }
    try {
      const resolved = await artworkApi.resolveYouTube(videoId);
      if (currentRequest === requestId) {
        source.value = resolved;
      }
    } catch {
      if (currentRequest === requestId) {
        source.value = null;
      }
    }
  },
  { immediate: true },
);

function clearBrokenImage(): void {
  source.value = null;
}
</script>

<template>
  <img
    v-if="source"
    :src="source"
    alt=""
    decoding="async"
    draggable="false"
    referrerpolicy="no-referrer"
    @error="clearBrokenImage"
  />
</template>
