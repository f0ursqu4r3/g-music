<script setup lang="ts">
import { computed, ref, watch } from "vue";

const props = defineProps<{
  videoId: string | null | undefined;
}>();

type ThumbnailQuality = "maxresdefault" | "hqdefault";

const quality = ref<ThumbnailQuality>("maxresdefault");
const unavailable = ref(false);
const youtubeVideoId = computed(() => {
  const videoId = props.videoId?.trim() ?? "";
  return /^[A-Za-z0-9_-]{11}$/.test(videoId) ? videoId : null;
});
const source = computed(() => {
  if (!youtubeVideoId.value || unavailable.value) {
    return null;
  }

  return `https://i.ytimg.com/vi/${youtubeVideoId.value}/${quality.value}.jpg`;
});

watch(
  () => props.videoId,
  () => {
    quality.value = "maxresdefault";
    unavailable.value = false;
  },
);

function useFallback(): void {
  if (quality.value === "maxresdefault") {
    quality.value = "hqdefault";
    return;
  }

  unavailable.value = true;
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
    @error="useFallback"
  />
</template>
