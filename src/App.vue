<script setup lang="ts">
import { emitTo, listen } from '@tauri-apps/api/event'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
import { computed, defineAsyncComponent, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { DialogRoot, DialogContent, DialogOverlay, DialogTitle } from 'reka-ui'
import { toast } from 'vue-sonner'

import {
  type CommandError,
  type ImportProgress,
  type MetadataRefreshSnapshot,
  type PlaybackSnapshot,
  type PlaybackTransport,
  type Playlist,
  windowApi,
  youtubeAuthApi,
} from '@/api'
import { hasOpenOverlay, isPaletteShortcut } from '@/lib/command-palette'
import {
  handoffCommandPalette,
  PALETTE_ACK_EVENT,
  PALETTE_REQUEST_EVENT,
  showAppWindow,
} from '@/lib/app-windows'
import { resolvePlaybackHotkey } from '@/lib/hotkeys'
import { useTheme, themes } from '@/lib/theme'
import { resolveMockWindowView } from '@/lib/window-view'
import { trackWindowCursor } from '@/lib/window-cursor'
import { playerDimensions } from '@/presentation'

import { Toaster } from '@/components/ui/sonner'
import { usePlayback } from '@/composables/usePlayback'

const ArtworkWindow = defineAsyncComponent(() => import('@/components/ArtworkWindow.vue'))
const ImportWindow = defineAsyncComponent(() => import('@/components/ImportWindow.vue'))
const LibraryWindow = defineAsyncComponent(() => import('@/components/LibraryWindow.vue'))
const MiniWindow = defineAsyncComponent(() => import('@/components/MiniWindow.vue'))
const QueueWindow = defineAsyncComponent(() => import('@/components/QueueWindow.vue'))
const SettingsWindow = defineAsyncComponent(() => import('@/components/SettingsWindow.vue'))

const playback = usePlayback()
const view = resolveMockWindowView(window.location.search)
const paletteRequest = ref(0)
let smartRefreshInterval: number | undefined
let paletteHandoffPending = false
const paletteHandoffAbort = new AbortController()
const queueExpanded = ref(false)
const keyboardShortcutsOpen = ref(false)
const theme = useTheme()
const windowError = ref('')
const isWindowFocused = ref(true)
const isCursorWithinWindow = ref<boolean | null>(null)
let stopWindowCursor: (() => void) | undefined
let isMounted = false
let unlistenWindowFocus: (() => void) | undefined
let playbackSyncInterval: number | undefined
const unlisteners: (() => void)[] = []
const failedSubscriptions = reactive(
  new Map<string, { retry: () => Promise<void>; message: string }>(),
)
const startupAttempted = ref(false)
let retryWindowAction: (() => Promise<void>) | undefined
let errorToastId: string | number | undefined
const isRetryingError = ref(false)
const isReconnectingYouTube = ref(false)
const youtubeReconnectError = ref('')

const isViewLoaded = computed(() =>
  view === 'library'
    ? Boolean(playback.library.value && playback.transport.value)
    : Boolean(playback.snapshot.value),
)
const playbackSnapshot = computed<PlaybackSnapshot>(
  () =>
    playback.snapshot.value ?? {
      currentItem: null,
      positionMs: 0,
      queue: [],
      status: 'paused',
      volumePercent: 0,
    },
)

const displayedError = computed(
  () =>
    windowError.value ||
    failedSubscriptions.values().next().value?.message ||
    playback.errorMessage.value ||
    (startupAttempted.value && !isViewLoaded.value
      ? 'Music could not load. Use Retry to load this window.'
      : ''),
)
const canDismissError = computed(
  () => isViewLoaded.value && !failedSubscriptions.size && !isRetryingError.value,
)
const needsYouTubeReconnect = computed(
  () =>
    !windowError.value &&
    !failedSubscriptions.size &&
    Boolean(playback.errorMessage.value) &&
    playback.errorCode.value === 'youtube_session_expired',
)
watch(needsYouTubeReconnect, (needed) => {
  if (!needed) {
    isReconnectingYouTube.value = false
    youtubeReconnectError.value = ''
  }
})
watch(
  [
    displayedError,
    canDismissError,
    isRetryingError,
    needsYouTubeReconnect,
    isReconnectingYouTube,
    youtubeReconnectError,
    () => playback.isUpdating.value,
  ],
  ([message, dismissible]) => {
    if (!message) {
      if (!isRetryingError.value && errorToastId !== undefined) {
        toast.dismiss(errorToastId)
        errorToastId = undefined
      }
      return
    }
    errorToastId = toast.error(message, {
      id: errorToastId,
      duration: Infinity,
      dismissible,
      closeButton: dismissible,
      description:
        youtubeReconnectError.value ||
        (needsYouTubeReconnect.value && isReconnectingYouTube.value
          ? 'Complete sign-in in the YouTube window, then select Save session and retry.'
          : undefined),
      action: {
        label: isRetryingError.value
          ? 'Retrying…'
          : needsYouTubeReconnect.value
            ? isReconnectingYouTube.value
              ? 'Save session and retry'
              : 'Reconnect YouTube'
            : 'Retry',
        onClick: (event) => {
          // Sonner otherwise removes the toast before an async retry can fail.
          event.preventDefault()
          if (!isRetryingError.value && !playback.isUpdating.value) void retryError()
        },
      },
      onDismiss: () => {
        if (!canDismissError.value || displayedError.value !== message) return
        windowError.value = ''
        playback.errorMessage.value = ''
      },
    })
  },
  { flush: 'post' },
)

async function retryError(): Promise<void> {
  isRetryingError.value = true
  try {
    if (needsYouTubeReconnect.value) {
      youtubeReconnectError.value = ''
      try {
        if (!isReconnectingYouTube.value) {
          await youtubeAuthApi.openLogin()
          if (isMounted) isReconnectingYouTube.value = true
          return
        }
        await youtubeAuthApi.saveSession()
        if (!isMounted) return
        isReconnectingYouTube.value = false
      } catch (error) {
        if (isMounted)
          youtubeReconnectError.value =
            typeof error === 'object' && error !== null && 'message' in error
              ? String(error.message)
              : 'Could not reconnect YouTube. Complete sign-in and try again.'
        return
      }
    }
    await retryOperation()
  } finally {
    isRetryingError.value = false
  }
}

const favoriteTrackIds = computed(
  () =>
    playback.library.value?.playlists?.find((playlist) => playlist.id === 'favorites')?.trackIds ??
    [],
)
async function initializePlayback(): Promise<void> {
  if (failedSubscriptions.has('import-progress')) await playback.refresh()
  else await playback.initialize()
}

async function retryOperation(): Promise<void> {
  const windowAction = retryWindowAction
  retryWindowAction = undefined
  windowError.value = ''
  if (windowAction) await windowAction()
  else if (failedSubscriptions.size) await retrySubscriptions()
  else if (!isViewLoaded.value) {
    playback.errorMessage.value = ''
    await initializePlayback()
  } else await playback.retry()
}

async function persistPlaylist(playlist: Playlist): Promise<void> {
  try {
    await playback.upsertPlaylist(playlist)
  } catch (error) {
    playback.reportError(error, () => persistPlaylist(playlist))
  }
}
function cycleTheme(): void {
  const currentIndex = themes.indexOf(theme.value)
  theme.value = themes[(currentIndex + 1) % themes.length]
}

async function toggleQueue(): Promise<void> {
  await resizeQueue(!queueExpanded.value)
}

async function resizeQueue(expanded: boolean): Promise<void> {
  windowError.value = ''

  const size = playerDimensions(expanded)
  const currentWindow = getCurrentWindow()
  let unlocked = false

  try {
    await currentWindow.setResizable(true)
    unlocked = true
    await currentWindow.setSize(new LogicalSize(size.width, size.height))
    if (isMounted) queueExpanded.value = expanded
  } catch (error) {
    if (!isMounted) return
    retryWindowAction = () => resizeQueue(expanded)
    windowError.value = error instanceof Error ? error.message : 'Could not resize the mini player.'
  } finally {
    if (unlocked) {
      try {
        await currentWindow.setResizable(false)
      } catch (error) {
        if (isMounted) {
          retryWindowAction = () => resizeQueue(expanded)
          windowError.value =
            error instanceof Error ? error.message : 'Could not lock the mini player size.'
        }
      }
    }
  }
}

async function closeMiniPlayer(): Promise<void> {
  try {
    await getCurrentWindow().close()
  } catch (error) {
    playback.reportError(error, closeMiniPlayer)
  }
}

async function openImportWindow(): Promise<void> {
  windowError.value = ''

  try {
    await windowApi.showImport()
  } catch (error) {
    retryWindowAction = openImportWindow
    windowError.value =
      error instanceof Error ? error.message : 'Could not open the Import Music window.'
  }
}

async function openArtworkDestination(view: 'queue' | 'library'): Promise<void> {
  try {
    await showAppWindow(view)
  } catch (error) {
    playback.reportError(error, () => openArtworkDestination(view))
  }
}

async function openPaletteFromCompactWindow(): Promise<void> {
  if (paletteHandoffPending) return
  paletteHandoffPending = true
  try {
    await handoffCommandPalette(getCurrentWindow().label, paletteHandoffAbort.signal)
  } catch (error) {
    if (isMounted) playback.reportError(error, openPaletteFromCompactWindow)
  } finally {
    paletteHandoffPending = false
  }
}

function handleKeyboard(event: KeyboardEvent): void {
  if (
    view !== 'library' &&
    isPaletteShortcut(event) &&
    !hasOpenOverlay() &&
    !keyboardShortcutsOpen.value
  ) {
    event.preventDefault()
    void openPaletteFromCompactWindow()
    return
  }
  const target = event.target
  if (
    event.defaultPrevented ||
    keyboardShortcutsOpen.value ||
    document.querySelector(
      '[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"]',
    )
  )
    return
  const isTextEditing =
    target instanceof HTMLElement &&
    Boolean(
      target.isContentEditable ||
      target.closest(
        'input, textarea, select, button, a, [role="button"], [role="checkbox"], [role="slider"], [role="menu"], [role="menuitem"], [role="menuitemradio"], [role="menuitemcheckbox"], [role="dialog"], [role="alertdialog"]',
      ),
    )
  const action = resolvePlaybackHotkey(
    event.key,
    isTextEditing,
    event.metaKey || event.ctrlKey || event.altKey,
  )

  if (action === 'toggle') {
    event.preventDefault()
    void playback.toggle()
  } else if (action === 'previous') {
    event.preventDefault()
    void playback.previous()
  } else if (action === 'next') {
    event.preventDefault()
    void playback.next()
  } else if (action === 'toggleMute') {
    event.preventDefault()
    void playback.toggleMute()
  } else if (
    view === 'mini' &&
    !isTextEditing &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    event.key.toLowerCase() === 'q'
  ) {
    event.preventDefault()
    void toggleQueue()
  }
}

async function trackArtworkWindowFocus(): Promise<void> {
  const currentWindow = getCurrentWindow()

  try {
    const focused = await currentWindow.isFocused()
    if (!isMounted) return
    isWindowFocused.value = focused
    const unlisten = await currentWindow.onFocusChanged(({ payload }) => {
      if (isMounted) {
        isWindowFocused.value = payload
        if (
          payload &&
          view === 'library' &&
          playback.library.value?.playlists.some((playlist) => playlist.smart)
        )
          void playback.refresh()
      }
    })

    if (isMounted) {
      unlistenWindowFocus = unlisten
    } else {
      unlisten()
    }
  } catch {
    if (isMounted) isWindowFocused.value = true
  }
}

async function subscribe<T>(name: string, handler: (payload: T) => void): Promise<void> {
  try {
    const unlisten = await listen<T>(name, ({ payload }) => {
      if (isMounted) handler(payload)
    })
    if (isMounted) unlisteners.push(unlisten)
    else unlisten()
    failedSubscriptions.delete(name)
  } catch (error) {
    if (isMounted) {
      playback.reportError(error, retrySubscriptions)
      failedSubscriptions.set(name, {
        retry: () => subscribe(name, handler),
        message: playback.errorMessage.value,
      })
    }
  }
}

async function retrySubscriptions(): Promise<void> {
  playback.errorMessage.value = ''
  await Promise.all([...failedSubscriptions.values()].map(({ retry }) => retry()))
  if (isMounted) await initializePlayback()
}

onMounted(async () => {
  isMounted = true
  if (view === 'artwork' && isTauri()) {
    stopWindowCursor = trackWindowCursor((inside) => {
      isCursorWithinWindow.value = inside
    })
  }
  window.addEventListener('keydown', handleKeyboard)
  await Promise.all([
    subscribe<PlaybackSnapshot>('playback-updated', (payload) => playback.applySnapshot(payload)),
    subscribe<PlaybackTransport>('playback-transport-updated', (payload) =>
      playback.applyTransportEvent(payload),
    ),
    subscribe('library-updated', () => {
      void playback.refresh()
    }),
    subscribe<CommandError>('playback-error', (payload) =>
      playback.reportError(payload, playback.retryPlayback),
    ),
    subscribe<ImportProgress>('import-progress', (payload) =>
      playback.updateImportProgress(payload),
    ),
    subscribe<MetadataRefreshSnapshot>('metadata-refresh-progress', (payload) =>
      playback.updateMetadataRefreshes(payload),
    ),
    ...(view === 'library'
      ? [
          subscribe<{ requestId: string; sourceLabel: string }>(
            PALETTE_REQUEST_EVENT,
            (payload) => {
              const paletteAlreadyOpen = Boolean(document.querySelector('[data-command-search]'))
              if (
                !isViewLoaded.value ||
                (hasOpenOverlay() && !paletteAlreadyOpen) ||
                keyboardShortcutsOpen.value
              )
                return
              if (
                !payload ||
                typeof payload.requestId !== 'string' ||
                !['main', 'artwork', 'queue', 'mini-player', 'settings', 'import'].includes(
                  payload.sourceLabel,
                )
              )
                return
              if (!paletteAlreadyOpen) paletteRequest.value++
              void emitTo(payload.sourceLabel, PALETTE_ACK_EVENT, {
                requestId: payload.requestId,
              }).catch((error) => playback.reportError(error))
            },
          ),
        ]
      : []),
    subscribe('show-keyboard-shortcuts', () => {
      keyboardShortcutsOpen.value = true
    }),
  ])
  if (!isMounted) return
  await initializePlayback()
  if (!isMounted) return
  startupAttempted.value = true
  if (view !== 'settings')
    playbackSyncInterval = window.setInterval(() => {
      void playback.sync()
    }, 500)
  if (view === 'artwork' || view === 'library') void trackArtworkWindowFocus()
  if (view === 'library')
    smartRefreshInterval = window.setInterval(() => {
      if (
        isWindowFocused.value &&
        playback.library.value?.playlists.some((playlist) =>
          playlist.smart?.rules.some((rule) => rule.field === 'lastPlayedDays'),
        )
      )
        void playback.refresh()
    }, 60_000)
})

onUnmounted(() => {
  isMounted = false
  paletteHandoffAbort.abort()
  if (errorToastId !== undefined) toast.dismiss(errorToastId)
  unlistenWindowFocus?.()
  stopWindowCursor?.()
  if (smartRefreshInterval !== undefined) window.clearInterval(smartRefreshInterval)
  unlisteners.splice(0).forEach((unlisten) => unlisten())
  if (playbackSyncInterval !== undefined) {
    window.clearInterval(playbackSyncInterval)
  }
  window.removeEventListener('keydown', handleKeyboard)
})
</script>

<template>
  <div class="relative min-h-screen w-full" :data-view="view" :data-queue-expanded="queueExpanded">
    <Toaster
      position="top-right"
      :offset="{ top: 40, right: 16, left: 16, bottom: 16 }"
      :mobile-offset="{ top: 40, right: 16, left: 16, bottom: 16 }"
      :toast-options="{
        closeButtonAriaLabel: 'Dismiss error',
        classes: { toast: 'rounded-2xl', title: 'select-text break-words' },
      }"
    />
    <SettingsWindow v-if="view === 'settings'" :theme="theme" @update:theme="theme = $event" />

    <section
      v-else-if="!isViewLoaded"
      class="grid min-h-screen content-center gap-3.5 bg-(--glass-window) p-12"
      :aria-label="displayedError ? 'Music window unavailable' : 'Loading music window'"
    >
      <p v-if="displayedError" class="text-(--text)">
        Music could not load. Use Retry to load this window.
      </p>
      <template v-else>
        <span class="block h-3 w-[18%] rounded-full bg-(--surface-muted)" />
        <span class="block h-3 w-[48%] rounded-full bg-(--surface-muted)" />
        <span class="block h-3 w-[32%] rounded-full bg-(--surface-muted)" />
      </template>
    </section>

    <LibraryWindow
      v-else-if="view === 'library'"
      :playlists="playback.library.value?.playlists"
      :tracks="playback.library.value?.tracks"
      :transport="playback.transport.value ?? undefined"
      :is-starting="playback.isStarting.value"
      :is-updating="playback.isUpdating.value"
      :metadata-refreshes="playback.metadataRefreshes.value"
      :import-progress="playback.importProgress.value"
      :is-importing="playback.isImporting.value"
      :is-cancelling="playback.isCancelling?.value"
      @cancel-import="playback.cancelYouTubeImport"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @toggle-shuffle="playback.toggleShuffle"
      @cycle-repeat-mode="playback.cycleRepeatMode"
      @play-track="(queueIds, id) => playback.playTrack(id, queueIds)"
      @play-next="playback.playNext"
      @add-to-queue="playback.addToQueue"
      @seek="playback.seek"
      @set-volume="playback.setVolume"
      @toggle-mute="playback.toggleMute"
      @open-import="openImportWindow"
      :save-playlist="playback.upsertPlaylist"
      :preview-smart-playlist="playback.previewSmartPlaylist"
      :freeze-smart-playlist="playback.freezeSmartPlaylist"
      :run-palette-tracks="playback.runPaletteTracks"
      :open-window-action="showAppWindow"
      :palette-request="paletteRequest"
      @upsert-playlist="persistPlaylist"
      :save-metadata="playback.updateTracksMetadata"
      :delete-playlist-action="playback.deletePlaylist"
      :reset-metadata="playback.resetTrackMetadata"
      :is-retrying-metadata="playback.isRetryingMetadata?.value"
      @retry-metadata-refreshes="playback.retryMetadataRefreshes"
      @reorder-playlists="playback.reorderPlaylists"
      @remove-tracks="playback.removeTracks"
      @toggle-favorite="playback.toggleFavorite"
    />

    <ImportWindow
      v-else-if="view === 'import'"
      :is-importing="playback.isImporting.value"
      :is-cancelling="playback.isCancelling?.value"
      :progress="playback.importProgress.value"
      :metadata-refreshes="playback.metadataRefreshes.value"
      :is-retrying-metadata="playback.isRetryingMetadata?.value"
      @retry-metadata="playback.retryMetadataRefreshes"
      @cancel-import="playback.cancelYouTubeImport"
      @retry="retryOperation"
      @import-youtube-urls="playback.importYouTubeUrls"
    />

    <ArtworkWindow
      v-else-if="view === 'artwork'"
      :snapshot="playbackSnapshot"
      :is-updating="playback.isUpdating.value"
      :is-window-focused="isWindowFocused"
      :is-cursor-within-window="isCursorWithinWindow"
      @set-volume="playback.setVolume"
      @toggle-mute="playback.toggleMute"
      @open-queue="openArtworkDestination('queue')"
      @open-library="openArtworkDestination('library')"
      :is-starting="playback.isStarting.value"
      :favorite-track-ids="favoriteTrackIds"
      @toggle-shuffle="playback.toggleShuffle"
      @cycle-repeat-mode="playback.cycleRepeatMode"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @seek="playback.seek"
      @toggle-favorite="playback.toggleFavorite"
    />

    <QueueWindow
      v-else-if="view === 'queue'"
      :queue="playbackSnapshot.queue"
      :clear-queue="playback.clearQueue"
      :save-playlist="playback.upsertPlaylist"
      :playback-order="playbackSnapshot.playbackOrder"
      :current-item-id="playbackSnapshot.currentItem?.id"
      :status="playbackSnapshot.status"
      :shuffle-enabled="playbackSnapshot.shuffleEnabled"
      :position-ms="playbackSnapshot.positionMs"
      :is-starting="playback.isStarting.value"
      :is-updating="playback.isUpdating.value"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @play-track="
        (id) =>
          playback.playTrack(
            id,
            playbackSnapshot.queue.map((item) => item.id),
          )
      "
      @move="playback.moveQueueItem"
      @remove="playback.removeQueueItem"
    />

    <MiniWindow
      v-else
      :snapshot="playbackSnapshot"
      :is-updating="playback.isUpdating.value"
      :is-starting="playback.isStarting.value"
      :favorite-track-ids="
        playback.library.value?.playlists?.find((playlist) => playlist.id === 'favorites')
          ?.trackIds ?? []
      "
      :queue-expanded="queueExpanded"
      :theme="theme"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @seek="playback.seek"
      @set-volume="playback.setVolume"
      @toggle-mute="playback.toggleMute"
      @toggle-shuffle="playback.toggleShuffle"
      @cycle-repeat-mode="playback.cycleRepeatMode"
      @toggle-favorite="playback.toggleFavorite"
      @move="playback.moveQueueItem"
      @remove="playback.removeQueueItem"
      @toggle-queue="toggleQueue"
      @toggle-theme="cycleTheme"
      @close="closeMiniPlayer"
    />

    <DialogRoot v-model:open="keyboardShortcutsOpen">
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60" />
      <DialogContent
        :aria-describedby="undefined"
        class="fixed left-1/2 top-1/2 z-50 max-h-[calc(100dvh-2rem)] w-[min(23rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-lg border border-(--line) bg-(--menu-surface) p-5 text-(--text) shadow-xl"
      >
        <header class="flex items-center justify-between gap-4">
          <DialogTitle class="text-lg font-semibold text-(--text)">
            Keyboard Shortcuts
          </DialogTitle>
          <button
            class="rounded-md px-2 py-1 text-sm text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text)"
            type="button"
            aria-label="Close keyboard shortcuts"
            @click="keyboardShortcutsOpen = false"
          >
            Esc
          </button>
        </header>
        <dl class="mt-5 grid grid-cols-[1fr_auto] gap-x-6 gap-y-3 text-sm">
          <dt class="text-(--muted-text)">Command palette</dt>
          <dd class="font-mono text-(--text)">⌘ K / Ctrl K</dd>
          <dt class="text-(--muted-text)">Library search</dt>
          <dd class="font-mono text-(--text)">⌘ F / Ctrl F</dd>
          <dt class="text-(--muted-text)">Play or pause</dt>
          <dd class="font-mono text-(--text)">Space</dd>
          <dt class="text-(--muted-text)">Previous track</dt>
          <dd class="font-mono text-(--text)">⌘ ← / J</dd>
          <dt class="text-(--muted-text)">Next track</dt>
          <dd class="font-mono text-(--text)">⌘ → / K</dd>
          <dt class="text-(--muted-text)">Mute</dt>
          <dd class="font-mono text-(--text)">M</dd>
          <dt class="text-(--muted-text)">Import music</dt>
          <dd class="font-mono text-(--text)">⌘ I</dd>
          <dt class="text-(--muted-text)">Library windows</dt>
          <dd class="font-mono text-(--text)">⌘ 1–4</dd>
        </dl>
      </DialogContent>
    </DialogRoot>
  </div>
</template>
