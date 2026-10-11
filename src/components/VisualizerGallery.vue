<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import {
  DialogRoot,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from 'reka-ui'
import { Check, X } from 'lucide-vue-next'
import { visualizerPresets, type VisualizerPresetId } from '@/lib/visualizer/presets'
import { createPresetPreviews } from '@/lib/visualizer/previews'

defineProps<{ selected: VisualizerPresetId }>()
const emit = defineEmits<{ select: [id: VisualizerPresetId]; close: [] }>()
const family = ref('All')
const families = ['All', 'Essential', 'Atmosphere', 'Cosmos', 'Geometry', 'Electric']
const presets = computed(() =>
  visualizerPresets.filter((preset) => family.value === 'All' || preset.family === family.value),
)
const previews = reactive<Partial<Record<VisualizerPresetId, string>>>({})
const portalTarget =
  document.fullscreenElement instanceof HTMLElement ? document.fullscreenElement : document.body
const returnFocus = document.activeElement
let stopPreviews: (() => void) | undefined
onMounted(() => {
  stopPreviews = createPresetPreviews((id, url) => {
    previews[id] = url
  })
})
onUnmounted(() => stopPreviews?.())
function restoreFocus(event: Event) {
  event.preventDefault()
  if (returnFocus instanceof HTMLElement && returnFocus.isConnected) returnFocus.focus()
}
</script>

<template>
  <DialogRoot :open="true" @update:open="!$event && emit('close')">
    <DialogPortal :to="portalTarget">
      <DialogOverlay class="gallery-overlay" />
      <DialogContent class="preset-gallery" @close-auto-focus="restoreFocus">
        <header class="gallery-heading">
          <div>
            <DialogTitle class="gallery-title">Choose your atmosphere</DialogTitle>
            <DialogDescription class="gallery-description"
              >20 visualizers. Every one in 4K.</DialogDescription
            >
          </div>
          <button class="gallery-close" aria-label="Close visualizers" @click="emit('close')">
            <X :size="20" />
          </button>
        </header>
        <div class="gallery-filters" role="group" aria-label="Visualizer categories">
          <button
            v-for="item in families"
            :key="item"
            :aria-pressed="family === item"
            @click="family = item"
          >
            {{ item }}
          </button>
        </div>
        <div class="gallery-grid">
          <button
            v-for="preset in presets"
            :key="preset.id"
            class="preset-card"
            :style="{ '--preset-accent': preset.accent }"
            :aria-label="`Use ${preset.name}`"
            :aria-pressed="selected === preset.id"
            @click="emit('select', preset.id)"
          >
            <div class="preset-image">
              <img
                v-if="previews[preset.id]"
                :src="previews[preset.id]"
                alt=""
                width="320"
                height="200"
              />
              <span v-else class="preset-placeholder" aria-hidden="true" />
              <span v-if="selected === preset.id" class="preset-check"><Check :size="13" /></span>
            </div>
            <span class="preset-name">{{ preset.name }}</span>
            <span class="preset-description">{{ preset.description }}</span>
          </button>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
.gallery-overlay {
  position: fixed;
  inset: 0;
  z-index: 80;
  background: #03050ace;
  backdrop-filter: blur(16px);
}
.preset-gallery {
  position: fixed;
  inset: 50% auto auto 50%;
  transform: translate(-50%, -50%);
  z-index: 81;
  width: min(900px, calc(100vw - 40px));
  max-height: calc(100dvh - 40px);
  display: flex;
  flex-direction: column;
  background: #0c1019;
  border: 1px solid #ffffff1a;
  border-radius: 18px;
  color: #eaf0fa;
  box-shadow: 0 32px 100px #0009;
  overflow: hidden;
}
.gallery-heading {
  padding: 24px 26px 16px;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.gallery-title {
  margin: 0;
  font-size: 24px;
  font-weight: 500;
  letter-spacing: -0.035em;
  line-height: 1.2;
}
.gallery-description {
  margin: 7px 0 0;
  color: #91a0b6;
  font-size: 12px;
}
.gallery-close {
  display: grid;
  place-items: center;
  padding: 5px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: #a7b5c9;
  cursor: pointer;
}
.gallery-filters {
  padding: 0 26px 16px;
  display: flex;
  gap: 5px;
  overflow-x: auto;
  flex-shrink: 0;
}
.gallery-filters button {
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 6px 10px;
  background: transparent;
  font-size: 11px;
  color: #9cabc0;
  cursor: pointer;
  white-space: nowrap;
}
.gallery-filters button[aria-pressed='true'] {
  color: #f2f5fc;
  background: #ffffff12;
  border-color: #ffffff12;
}
.gallery-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 20px 14px;
  overflow-y: auto;
  min-height: 0;
  padding: 0 26px 26px;
  overscroll-behavior: contain;
}
.preset-card {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  border: 0;
  padding: 0;
  background: transparent;
  text-align: left;
  color: inherit;
  cursor: pointer;
  border-radius: 8px;
}
.preset-image {
  position: relative;
  aspect-ratio: 1.6;
  overflow: hidden;
  border: 1px solid #ffffff12;
  border-radius: 8px;
  background: #050910;
  transition: border-color 120ms;
}
.preset-image img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.preset-placeholder {
  position: absolute;
  inset: 25%;
  border-radius: 50%;
  background: var(--preset-accent);
  opacity: 0.25;
  filter: blur(25px);
}
.preset-card:hover .preset-image,
.preset-card[aria-pressed='true'] .preset-image {
  border-color: var(--preset-accent);
}
.preset-check {
  position: absolute;
  right: 7px;
  top: 7px;
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  color: #0c1019;
  background: var(--preset-accent);
}
.preset-name {
  margin-top: 9px;
  font-size: 13px;
  font-weight: 500;
}
.preset-description {
  margin-top: 4px;
  font-size: 10px;
  line-height: 1.45;
  color: #8d9bb0;
}
button:focus-visible {
  outline: 2px solid #a8caff;
  outline-offset: 3px;
}
@media (max-width: 700px) {
  .gallery-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
@media (max-width: 530px) {
  .preset-gallery {
    width: calc(100vw - 20px);
    max-height: calc(100dvh - 20px);
    border-radius: 12px;
  }
  .gallery-heading {
    padding: 16px 16px 12px;
  }
  .gallery-title {
    font-size: 20px;
  }
  .gallery-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    padding: 0 16px 16px;
  }
  .gallery-filters {
    padding: 0 16px 12px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .preset-image {
    transition: none;
  }
}
</style>
