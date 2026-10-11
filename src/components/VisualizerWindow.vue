<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import {
  ChevronLeft,
  ChevronRight,
  Film,
  Grid2X2,
  Maximize2,
  Minimize2,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Waves,
} from 'lucide-vue-next'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { PlaybackSnapshot } from '@/api'
import { visualizerApi } from '@/lib/visualizer/api'
import { VisualizerTimeline } from '@/lib/visualizer/timeline'
import { AudioDynamics } from '@/lib/visualizer/dynamics'
import { VisualizerVideo, type VideoState } from '@/lib/visualizer/video'
import { VisualizerRenderer, fallbackPalette, type Palette } from '@/lib/visualizer/renderer'
import { loadSettings, saveSettings } from '@/lib/visualizer/settings'
import { artworkPalette, themePalette } from '@/lib/visualizer/palette'
import {
  adjacentPreset,
  visualizerPresets,
  type VisualizerPresetId,
} from '@/lib/visualizer/presets'
import VisualizerGallery from './VisualizerGallery.vue'

const props = defineProps<{ snapshot: PlaybackSnapshot }>()
const emit = defineEmits<{ togglePlay: []; previous: []; next: [] }>()
const native = visualizerApi.available()
const motion = window.matchMedia?.('(prefers-reduced-motion: reduce)')
const reducedMotion = ref(motion?.matches ?? false)
const settings = reactive(loadSettings(reducedMotion.value))
const galleryOpen = ref(false)
const selectedPreset = computed(() =>
  visualizerPresets.find((preset) => preset.id === settings.preset)!,
)
function selectPreset(id: VisualizerPresetId): void {
  settings.preset = id
  galleryOpen.value = false
}
const canvas = ref<HTMLCanvasElement>()
const panel = ref<HTMLElement>()
const status = ref('idle'),
  error = ref<string | null>(null),
  graphicsError = ref<string | null>(null)
const demo = ref(false),
  fullscreen = ref(false)
const track = computed(() => props.snapshot.currentItem)
const stateLabel = computed(() =>
  demo.value
    ? 'Demo · simulated audio'
    : !native
      ? 'Live audio requires the desktop app'
      : (error.value ??
        {
          idle: 'Play a track to begin',
          warming: 'Preparing audio',
          playing: 'Live audio',
          paused: 'Paused',
          buffering: 'Waiting for audio',
          error: 'Audio analysis is unavailable',
        }[status.value] ??
        'Waiting for audio'),
)
const timeline = new VisualizerTimeline()
const dynamics = new AudioDynamics()
const videoState = ref<VideoState>('off'),
  videoBlocked = ref(false)
const video = native
  ? new VisualizerVideo((state) => {
      videoState.value = state
    })
  : null
const videoNote = computed(() => {
  if (!settings.video || !native) return null
  if (videoBlocked.value) return 'This window cannot read video frames. The visualizer continues.'
  return {
    off: 'Play a track to show its video.',
    loading: 'Loading the music video…',
    ready: null,
    unavailable: 'No video is available for this track.',
  }[videoState.value]
})
let renderer: VisualizerRenderer | null = null,
  palette: Palette = fallbackPalette
let disposed = false,
  visible = !document.hidden,
  contextLost = false,
  running = false
let raf = 0,
  pollTimer: ReturnType<typeof setTimeout> | undefined
let nativeTimer: ReturnType<typeof setInterval> | undefined
let revision = 0,
  artworkRevision = 0,
  lastDraw = -Infinity
let leaseChain: Promise<void> = Promise.resolve()
const unlisten: (() => void)[] = []

watch(settings, saveSettings)
watch(
  () => [settings.video, track.value?.id] as const,
  ([enabled, id]) => video?.load(enabled && id ? id : null),
  { immediate: true },
)
watch(
  () => track.value?.id,
  async (id) => {
    const current = ++artworkRevision
    palette = themePalette()
    if (!native || !id) return
    const result = await artworkPalette(id)
    if (!disposed && current === artworkRevision) palette = result
  },
  { immediate: true },
)

function setLease(active: boolean): Promise<void> {
  // Serialize activation and release. A late activation cannot retain a closed window's lease.
  leaseChain = leaseChain.catch(() => {}).then(() => visualizerApi.setActive(active))
  return leaseChain
}
function stop(): void {
  const wasRunning = running
  running = false
  revision++
  cancelAnimationFrame(raf)
  clearTimeout(pollTimer)
  video?.pause()
  if (wasRunning && native) void setLease(false).catch(() => {})
}
async function poll(session: number): Promise<void> {
  if (!running || disposed || session !== revision) return
  const start = performance.now()
  try {
    const snapshot = await visualizerApi.inspect()
    if (!running || disposed || session !== revision) return
    timeline.observe(snapshot, start, performance.now())
    status.value = snapshot.status
    error.value = snapshot.error
  } catch (cause) {
    if (!running || disposed || session !== revision) return
    status.value = 'error'
    error.value = cause instanceof Error ? cause.message : String(cause)
  }
  if (running && !disposed && session === revision)
    pollTimer = setTimeout(() => {
      void poll(session)
    }, 80)
}
function draw(now: number): void {
  if (!running || disposed) return
  if (now - lastDraw >= 1000 / settings.fps - 1) {
    lastDraw = now
    const sample = demo.value
      ? {
          bands: Float32Array.from(
            { length: 16 },
            (_, i) =>
              0.08 +
              0.42 * Math.pow(Math.max(0, Math.sin(now / 700 + i * 0.39)), 3) +
              // A 120 BPM groove: kick on each beat, snare on the backbeat, hats on eighths.
              (i < 4 ? 0.45 * Math.exp(-(now % 500) / 70) : 0) +
              (i >= 6 && i < 12 ? 0.4 * Math.exp(-((now + 500) % 1000) / 60) : 0) +
              (i >= 12 ? 0.35 * Math.exp(-(now % 250) / 30) : 0),
          ),
          timeSeconds: now / 1000,
        }
      : timeline.sample(now, settings.offsetMs)
    const clip = settings.video && !demo.value ? video : null
    clip?.sync(timeline.playhead(now), settings.offsetMs, now)
    try {
      renderer?.draw(
        dynamics.update(sample, now),
        settings,
        palette,
        reducedMotion.value,
        panel.value?.offsetHeight ?? 0,
        clip,
      )
      videoBlocked.value = renderer?.videoBlocked ?? false
    } catch (cause) {
      graphicsError.value = String(cause)
      stop()
      return
    }
  }
  raf = requestAnimationFrame(draw)
}
async function start(): Promise<void> {
  if (running || disposed || !visible || contextLost || !renderer) return
  running = true
  const session = ++revision
  lastDraw = -Infinity
  timeline.reset()
  dynamics.reset()
  raf = requestAnimationFrame(draw)
  if (native) {
    try {
      await setLease(true)
      if (!disposed && running && session === revision) void poll(session)
    } catch (cause) {
      if (disposed || session !== revision) return
      error.value = String(cause)
      status.value = 'error'
    }
  }
}
function retryAudio(): void {
  stop()
  error.value = null
  status.value = 'warming'
  void start()
}
function createRenderer(): void {
  renderer?.dispose()
  renderer = null
  try {
    renderer = new VisualizerRenderer(canvas.value!)
    graphicsError.value = null
    void start()
  } catch (cause) {
    graphicsError.value = cause instanceof Error ? cause.message : String(cause)
  }
}
function visibilityChange(): void {
  visible = !document.hidden
  if (visible) void checkNativeVisibility()
  else stop()
}
async function checkNativeVisibility(): Promise<void> {
  if (disposed) return
  try {
    const minimized = native ? await getCurrentWindow().isMinimized() : false
    if (disposed) return
    visible = !document.hidden && !minimized
    if (visible) void start()
    else stop()
  } catch {
    if (!document.hidden) void start()
  }
}
function lost(event: Event): void {
  event.preventDefault()
  contextLost = true
  graphicsError.value = 'Graphics paused. Waiting for recovery.'
  stop()
  // Context loss destroys its GPU objects. Drop their handles before recovery.
  renderer = null
}
function restored(): void {
  if (disposed) return
  contextLost = false
  createRenderer()
}
function motionChange(): void {
  reducedMotion.value = motion?.matches ?? false
}
async function toggleFullscreen(): Promise<void> {
  try {
    if (native) {
      const win = getCurrentWindow()
      const current = await win.isFullscreen()
      await win.setFullscreen(!current)
      fullscreen.value = !current
    } else if (document.fullscreenElement) {
      await document.exitFullscreen()
      fullscreen.value = false
    } else {
      await canvas.value?.parentElement?.requestFullscreen()
      fullscreen.value = true
    }
  } catch {
    error.value = 'Fullscreen is unavailable.'
  }
}
function fullscreenChange(): void {
  if (!native) fullscreen.value = Boolean(document.fullscreenElement)
}
onMounted(() => {
  createRenderer()
  document.addEventListener('visibilitychange', visibilityChange)
  document.addEventListener('fullscreenchange', fullscreenChange)
  motion?.addEventListener('change', motionChange)
  if (native) {
    nativeTimer = setInterval(() => {
      void checkNativeVisibility()
    }, 500)
    void getCurrentWindow()
      .onResized(() => {
        void checkNativeVisibility()
      })
      .then((dispose) => {
        if (disposed) dispose()
        else unlisten.push(dispose)
      })
      .catch(() => {})
  }
})
onUnmounted(() => {
  disposed = true
  artworkRevision++
  stop()
  clearInterval(nativeTimer)
  renderer?.dispose()
  renderer = null
  video?.dispose()
  document.removeEventListener('visibilitychange', visibilityChange)
  document.removeEventListener('fullscreenchange', fullscreenChange)
  motion?.removeEventListener('change', motionChange)
  for (const dispose of unlisten) dispose()
})
</script>

<template>
  <main class="visualizer-window">
    <canvas
      ref="canvas"
      aria-label="Audio visualization"
      @webglcontextlost="lost"
      @webglcontextrestored="restored"
    />
    <header class="visualizer-header" data-tauri-drag-region>
      <div class="visualizer-wordmark"><Waves :size="17" /><span>Visualizer</span></div>
      <button
        :aria-label="fullscreen ? 'Exit fullscreen' : 'Enter fullscreen'"
        @click="toggleFullscreen"
      >
        <component :is="fullscreen ? Minimize2 : Maximize2" :size="17" />
      </button>
    </header>
    <section v-if="graphicsError" class="visualizer-message" role="status">
      <Waves :size="30" />
      <p>{{ graphicsError }}</p>
      <button v-if="!contextLost" @click="createRenderer">Retry graphics</button>
    </section>
    <div ref="panel" class="visualizer-bottom">
      <div class="visualizer-track-row">
        <div class="visualizer-track">
          <p
            class="visualizer-status"
            :class="{ 'is-live': native && status === 'playing' && !error }"
          >
            <span />{{ stateLabel }}
          </p>
          <h1>{{ track?.title ?? 'Music in motion' }}</h1>
          <p class="visualizer-artist">{{ track?.artist ?? 'A different way to listen.' }}</p>
        </div>
        <div class="visualizer-transport">
          <button :disabled="!native" aria-label="Previous track" @click="emit('previous')">
            <SkipBack :size="18" /></button
          ><button
            class="visualizer-play"
            :disabled="!native"
            :aria-label="snapshot.status === 'playing' ? 'Pause' : 'Play'"
            @click="emit('togglePlay')"
          >
            <component :is="snapshot.status === 'playing' ? Pause : Play" :size="19" /></button
          ><button :disabled="!native" aria-label="Next track" @click="emit('next')">
            <SkipForward :size="18" />
          </button>
        </div>
      </div>
      <div class="visualizer-controls">
        <button
          v-if="native && status === 'error'"
          class="visualizer-demo"
          aria-label="Retry audio analysis"
          @click="retryAudio"
        >
          Retry audio
        </button>
        <div class="visualizer-presets" aria-label="Visualization preset">
          <button
            aria-label="Previous visualizer"
            @click="settings.preset = adjacentPreset(settings.preset, -1)"
          >
            <ChevronLeft :size="15" />
          </button>
          <button
            class="visualizer-browse"
            aria-label="Browse visualizers"
            aria-haspopup="dialog"
            :aria-expanded="galleryOpen"
            @click="galleryOpen = true"
          >
            <span class="visualizer-preset-dot" :style="{ background: selectedPreset.accent }" />{{
              selectedPreset.name
            }}<Grid2X2 :size="13" />
          </button>
          <button
            aria-label="Next visualizer"
            @click="settings.preset = adjacentPreset(settings.preset, 1)"
          >
            <ChevronRight :size="15" />
          </button>
        </div>
        <label class="visualizer-intensity"
          >Intensity<input
            v-model.number="settings.intensity"
            aria-label="Intensity"
            type="range"
            min="0.25"
            max="2"
            step="0.05"
        /></label>
        <label class="visualizer-offset" title="Positive values delay the visual response."
          >Sync<input
            v-model.number="settings.offsetMs"
            aria-label="Sync offset in milliseconds"
            type="number"
            min="-1000"
            max="1000"
            step="20"
            @change="
              settings.offsetMs = Math.min(1000, Math.max(-1000, Number(settings.offsetMs) || 0))
            "
          /><span>ms</span></label
        >
        <label class="visualizer-fps"
          ><select v-model.number="settings.fps" aria-label="Frame rate">
            <option :value="60">60 fps</option>
            <option :value="30">30 fps</option>
          </select></label
        >
        <label
          ><select
            v-model="settings.quality"
            aria-label="Render quality"
            title="4K renders at up to 3840 × 2160. Display uses the screen resolution."
          >
            <option value="4k">4K</option>
            <option value="display">Display</option>
          </select></label
        >
        <button
          class="visualizer-demo visualizer-video"
          :aria-pressed="settings.video"
          :disabled="!native"
          :title="
            native
              ? 'Show the YouTube video under the visualization'
              : 'Video requires the desktop app'
          "
          @click="settings.video = !settings.video"
        >
          <Film :size="13" />Video
        </button>
        <label
          v-if="settings.video"
          class="visualizer-intensity"
          title="Strength of the visualizer over the video"
          >Overlay<input
            v-model.number="settings.videoOverlay"
            aria-label="Visualizer overlay strength"
            type="range"
            min="0"
            max="1"
            step="0.05"
        /></label>
        <button
          v-if="!native"
          class="visualizer-demo"
          :aria-label="demo ? 'Stop demo' : 'Start demo'"
          :aria-pressed="demo"
          @click="demo = !demo"
        >
          {{ demo ? 'Stop demo' : 'Try demo' }}
        </button>
      </div>
      <p v-if="videoNote" class="visualizer-note" role="status">{{ videoNote }}</p>
      <p v-else-if="reducedMotion" class="visualizer-note">
        Reduced motion is on. Ambient movement is frozen.
      </p>
      <p v-else class="visualizer-note">Positive sync values delay the visual response.</p>
    </div>
    <VisualizerGallery
      v-if="galleryOpen"
      :selected="settings.preset"
      @select="selectPreset"
      @close="galleryOpen = false"
    />
  </main>
</template>

<style scoped>
.visualizer-window {
  position: relative;
  width: 100%;
  height: 100dvh;
  min-height: 0;
  overflow: hidden;
  background: #070a0e;
  color: #e8edf0;
  font-family: inherit;
}
canvas {
  display: block;
  width: 100%;
  height: 100%;
}
.visualizer-header {
  position: absolute;
  inset: 0 0 auto;
  padding: 24px 28px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: linear-gradient(#070a0e88, transparent);
}
.visualizer-wordmark {
  display: flex;
  align-items: center;
  gap: 10px;
  color: #a5b4bd;
  font-size: 12px;
  letter-spacing: 0.08em;
}
button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 8px;
  background: transparent;
  color: #a5b4bd;
  cursor: pointer;
}
button:hover {
  background: #ffffff0d;
  color: #fff;
}
button:disabled {
  opacity: 0.35;
  cursor: default;
  pointer-events: none;
}
button:focus-visible,
input:focus-visible,
select:focus-visible {
  outline: 2px solid #97d5e5;
  outline-offset: 3px;
}
.visualizer-bottom {
  position: absolute;
  inset: auto 0 0;
  padding: 40px 28px 18px;
  background: linear-gradient(transparent, #070a0edb 30%, #070a0ef5);
}
.visualizer-track-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding-bottom: 22px;
}
.visualizer-track {
  min-width: 0;
}
h1 {
  font-size: clamp(20px, 3.3vw, 32px);
  line-height: 1.2;
  font-weight: 500;
  letter-spacing: -0.025em;
  margin: 9px 0 5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
p {
  margin: 0;
}
.visualizer-status {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 10px;
  color: #9baab2;
  letter-spacing: 0.035em;
}
.visualizer-status span {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #6b747e;
  flex-shrink: 0;
}
.visualizer-status.is-live span {
  background: #9bd8c5;
  box-shadow: 0 0 9px #9bd8c555;
}
.visualizer-artist {
  color: #8e9ca6;
  font-size: 12px;
}
.visualizer-transport {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
}
.visualizer-play {
  width: 42px;
  height: 42px;
  background: #edf3f0;
  color: #172129;
  border-radius: 50%;
}
.visualizer-play:hover {
  background: #fff;
  color: #172129;
}
.visualizer-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 18px;
  border-top: 1px solid #ffffff12;
  padding-top: 15px;
  font-size: 11px;
  color: #9baab2;
}
.visualizer-presets {
  display: flex;
  background: #ffffff05;
  border: 1px solid #ffffff0a;
  border-radius: 7px;
  padding: 3px;
}
.visualizer-presets button {
  padding: 5px 10px;
  font-size: 11px;
  border-radius: 4px;
}
.visualizer-browse {
  gap: 8px;
  min-width: 126px;
}
.visualizer-preset-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 0 10px currentColor;
}
.visualizer-presets button[aria-pressed='true'] {
  color: #e2ebee;
  background: #ffffff12;
}
label {
  display: flex;
  align-items: center;
  gap: 8px;
}
input[type='range'] {
  width: 74px;
  height: 3px;
  accent-color: #b3d4dc;
  cursor: pointer;
}
input[type='number'] {
  width: 48px;
  padding: 4px;
  border: 1px solid #ffffff14;
  border-radius: 4px;
  background: #ffffff05;
  color: #e2ebee;
  font-size: 11px;
}
select {
  background: #0f151b;
  color: #bcc9d0;
  border: 1px solid #ffffff14;
  border-radius: 5px;
  padding: 5px;
  font-size: 11px;
}
.visualizer-fps {
  margin-left: auto;
}
.visualizer-demo {
  font-size: 11px;
  border-color: #ffffff14;
  padding: 5px 8px;
}
.visualizer-video {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.visualizer-video[aria-pressed='true'] {
  border-color: #ffffff40;
  background: #ffffff14;
  color: #e8edf0;
}
.visualizer-note {
  font-size: 10px;
  color: #667681;
  margin-top: 10px;
}
.visualizer-message {
  position: absolute;
  inset: 27% 20px auto;
  display: flex;
  align-items: center;
  flex-direction: column;
  gap: 14px;
  text-align: center;
  color: #a5b4bd;
  font-size: 13px;
}
@media (max-width: 530px) {
  .visualizer-header {
    padding: 20px;
  }
  .visualizer-bottom {
    padding: 30px 20px 15px;
  }
  .visualizer-controls {
    gap: 12px;
  }
  .visualizer-intensity {
    flex: 1;
  }
  .visualizer-fps {
    margin-left: 0;
  }
}
@media (max-height: 400px) {
  .visualizer-header {
    padding: 14px 20px;
  }
  .visualizer-bottom {
    padding: 18px 20px 12px;
  }
  .visualizer-track-row {
    padding-bottom: 12px;
  }
  h1 {
    margin-top: 6px;
    font-size: 18px;
  }
  .visualizer-controls {
    padding-top: 10px;
    gap: 8px 12px;
  }
  .visualizer-note {
    display: none;
  }
}
</style>
