import { invoke } from "@tauri-apps/api/core";

export interface MediaItem {
  id: string;
  title: string;
  artist: string;
  durationMs: number;
}

export type PlaybackStatus = "paused" | "playing";

export interface PlaybackSnapshot {
  status: PlaybackStatus;
  currentItem: MediaItem | null;
  positionMs: number;
  volumePercent: number;
  queue: MediaItem[];
}

export interface CommandError {
  code: string;
  message: string;
}

export const playbackApi = {
  inspect: (): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("inspect_playback"),
  play: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>("play"),
  pause: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>("pause"),
  previous: (): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("previous"),
  next: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>("next"),
  seek: (positionMs: number): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("seek", { positionMs }),
  setVolume: (volumePercent: number): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("set_volume", { volumePercent }),
  moveQueueItem: (from: number, to: number): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("move_queue_item", { from, to }),
};
