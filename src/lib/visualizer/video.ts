import { convertFileSrc } from '@tauri-apps/api/core'

export type VideoState = 'off' | 'loading' | 'ready' | 'unavailable'
export interface Playhead {
  positionMs: number
  playing: boolean
}

const SYNC_INTERVAL_MS = 200
/** Beyond this drift a seek is cheaper than catching up with the playback rate. */
const SEEK_DRIFT_S = 0.3

export function isVideoId(id: string): boolean {
  return /^[A-Za-z0-9_-]{11}$/.test(id)
}

/**
 * A muted video that follows the audio playhead. Audio stays in mpv; this element only
 * supplies frames to WebGL through the app's `gmvideo` stream.
 */
export class VisualizerVideo {
  readonly element: HTMLVideoElement
  private id: string | null = null
  private requested: string | null = null
  private lastSync = -Infinity
  private frameDirty = true
  private watchesFrames = false

  constructor(private onState: (state: VideoState) => void) {
    const element = document.createElement('video')
    element.muted = true
    element.playsInline = true
    element.preload = 'auto'
    // WebGL may read only CORS-approved pixels. The stream replies with an open CORS header.
    element.crossOrigin = 'anonymous'
    element.disablePictureInPicture = true
    element.addEventListener('loadeddata', () => {
      if (this.id) this.onState('ready')
    })
    element.addEventListener('error', () => {
      if (this.id) this.onState('unavailable')
    })
    if ('requestVideoFrameCallback' in element) {
      this.watchesFrames = true
      const next = () => {
        this.frameDirty = true
        element.requestVideoFrameCallback(next)
      }
      element.requestVideoFrameCallback(next)
    }
    this.element = element
  }

  get ready(): boolean {
    return this.id !== null && this.element.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
  }

  get aspect(): number {
    const { videoWidth, videoHeight } = this.element
    return videoWidth > 0 && videoHeight > 0 ? videoWidth / videoHeight : 16 / 9
  }

  load(id: string | null): void {
    if (id === this.requested) return
    this.requested = id
    this.id = id && isVideoId(id) ? id : null
    this.lastSync = -Infinity
    this.frameDirty = true
    this.element.pause()
    if (!this.id) {
      this.element.removeAttribute('src')
      this.element.load()
      this.onState(id ? 'unavailable' : 'off')
      return
    }
    this.onState('loading')
    this.element.src = convertFileSrc(this.id, 'gmvideo')
  }

  /** Seeks on large drift, nudges the rate on small drift, and mirrors play and pause. */
  sync(playhead: Playhead | null, offsetMs: number, now: number): void {
    if (!this.id || now - this.lastSync < SYNC_INTERVAL_MS) return
    this.lastSync = now
    const video = this.element
    if (!playhead || video.readyState < HTMLMediaElement.HAVE_METADATA) {
      video.pause()
      return
    }
    // Positive offsets delay the visuals. The video follows the same calibration.
    const target = Math.max(0, (playhead.positionMs - offsetMs) / 1000)
    if (Number.isFinite(video.duration) && target >= video.duration) {
      video.pause()
      return
    }
    const drift = video.currentTime - target
    if (!video.seeking && Math.abs(drift) > SEEK_DRIFT_S) video.currentTime = target
    video.playbackRate = playhead.playing ? Math.min(1.07, Math.max(0.93, 1 - drift * 0.6)) : 1
    if (playhead.playing && video.paused) void video.play().catch(() => {})
    else if (!playhead.playing && !video.paused) video.pause()
  }

  /** True when the element has a frame the texture does not hold yet. */
  takeFrame(): boolean {
    if (!this.ready) return false
    if (!this.watchesFrames) return true
    const dirty = this.frameDirty
    this.frameDirty = false
    return dirty
  }

  pause(): void {
    this.element.pause()
  }

  dispose(): void {
    this.id = this.requested = null
    this.element.pause()
    this.element.removeAttribute('src')
    this.element.load()
  }
}
