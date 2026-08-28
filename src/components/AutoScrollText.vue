<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";

type TextElement = "h1" | "p" | "span";
type ScrollSpeed = "medium" | "slow";

interface Props {
  as?: TextElement;
  speed?: ScrollSpeed;
  text: string;
}

const props = withDefaults(defineProps<Props>(), {
  as: "p",
  speed: "medium",
});

const viewport = ref<HTMLElement>();
const content = ref<HTMLElement>();
const isOverflowing = ref(false);
let resizeObserver: ResizeObserver | undefined;

function measureOverflow(): void {
  const availableWidth = viewport.value?.clientWidth ?? 0;
  const contentWidth = content.value?.scrollWidth ?? 0;
  isOverflowing.value = availableWidth > 0 && contentWidth > availableWidth + 1;
}

function queueMeasurement(): void {
  void nextTick(measureOverflow);
}

watch(() => props.text, queueMeasurement);

onMounted(() => {
  window.addEventListener("resize", queueMeasurement);
  queueMeasurement();

  if (typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(queueMeasurement);
    if (viewport.value) resizeObserver.observe(viewport.value);
    if (content.value) resizeObserver.observe(content.value);
  }

  void document.fonts?.ready.then(queueMeasurement);
});

onUnmounted(() => {
  window.removeEventListener("resize", queueMeasurement);
  resizeObserver?.disconnect();
});
</script>

<template>
  <component
    :is="as"
    ref="viewport"
    class="auto-scroll-text"
    :data-overflowing="isOverflowing"
    :data-speed="speed"
    :title="text"
  >
    <span class="auto-scroll-track">
      <span class="auto-scroll-segment">
        <span ref="content" class="auto-scroll-content">{{ text }}</span>
      </span>
      <span
        v-if="isOverflowing"
        class="auto-scroll-segment"
        aria-hidden="true"
        >{{ text }}</span
      >
    </span>
  </component>
</template>

<style scoped>
.auto-scroll-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}

.auto-scroll-track {
  display: inline-flex;
  min-width: max-content;
  transform: translate3d(0, 0, 0);
}

.auto-scroll-content {
  display: inline-block;
}

.auto-scroll-text[data-overflowing="true"] {
  animation-name: auto-scroll-edge-fade;
  animation-duration: var(--auto-scroll-duration);
  animation-timing-function: linear;
  animation-iteration-count: infinite;
  mask-image: linear-gradient(
    90deg,
    black 0,
    black 0,
    black calc(100% - 0.8rem),
    transparent 100%
  );
  -webkit-mask-image: linear-gradient(
    90deg,
    black 0,
    black 0,
    black calc(100% - 0.8rem),
    transparent 100%
  );
}

.auto-scroll-text[data-overflowing="true"] .auto-scroll-segment {
  flex: none;
  padding-inline-end: 2.75rem;
}

.auto-scroll-text[data-overflowing="true"] .auto-scroll-track {
  animation-name: auto-scroll-loop;
  animation-duration: var(--auto-scroll-duration);
  animation-timing-function: linear;
  animation-iteration-count: infinite;
  will-change: transform;
}

.auto-scroll-text[data-speed="slow"] {
  --auto-scroll-duration: 15s;
}

.auto-scroll-text[data-speed="medium"] {
  --auto-scroll-duration: 12.5s;
}

.auto-scroll-text[data-overflowing="true"]:hover,
.auto-scroll-text:hover .auto-scroll-track {
  animation-play-state: paused;
}

@keyframes auto-scroll-edge-fade {
  0%,
  14%,
  86%,
  100% {
    mask-image: linear-gradient(
      90deg,
      black 0,
      black 0,
      black calc(100% - 0.8rem),
      transparent 100%
    );
    -webkit-mask-image: linear-gradient(
      90deg,
      black 0,
      black 0,
      black calc(100% - 0.8rem),
      transparent 100%
    );
  }

  15%,
  85% {
    mask-image: linear-gradient(
      90deg,
      transparent 0,
      black 0.8rem,
      black calc(100% - 0.8rem),
      transparent 100%
    );
    -webkit-mask-image: linear-gradient(
      90deg,
      transparent 0,
      black 0.8rem,
      black calc(100% - 0.8rem),
      transparent 100%
    );
  }
}

@keyframes auto-scroll-loop {
  0%,
  14% {
    transform: translate3d(0, 0, 0);
  }

  86%,
  100% {
    transform: translate3d(-50%, 0, 0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .auto-scroll-text[data-overflowing="true"] {
    animation: none;
    mask-image: none;
    -webkit-mask-image: none;
  }

  .auto-scroll-text[data-overflowing="true"] .auto-scroll-track {
    max-width: 100%;
    animation: none;
  }

  .auto-scroll-text[data-overflowing="true"] .auto-scroll-segment {
    overflow: hidden;
    padding-inline-end: 0;
    text-overflow: ellipsis;
  }

  .auto-scroll-segment[aria-hidden="true"] {
    display: none;
  }
}
</style>
