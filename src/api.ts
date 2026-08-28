import { invoke } from "@tauri-apps/api/core";

export interface MediaItem {
  id: string;
  title: string;
  artist: string;
  album?: string | null;
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

export interface YouTubeAuthStatus {
  connected: boolean;
}

export const playbackApi = {
  inspect: (): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("inspect_playback"),
  importYouTubeUrls: (urls: string[]): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("import_youtube_urls", { urls }),
  play: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>("play"),
  pause: (): Promise<PlaybackSnapshot> => invoke<PlaybackSnapshot>("pause"),
  playTrack: (id: string): Promise<PlaybackSnapshot> =>
    invoke<PlaybackSnapshot>("play_track", { id }),
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

export const windowApi = {
  showImport: (): Promise<void> => invoke<void>("show_import_window"),
};

export const youtubeAuthApi = {
  inspect: (): Promise<YouTubeAuthStatus> =>
    invoke<YouTubeAuthStatus>("inspect_youtube_auth"),
  openLogin: (): Promise<YouTubeAuthStatus> =>
    invoke<YouTubeAuthStatus>("open_youtube_login"),
  saveSession: (): Promise<YouTubeAuthStatus> =>
    invoke<YouTubeAuthStatus>("save_youtube_session"),
  disconnect: (): Promise<YouTubeAuthStatus> =>
    invoke<YouTubeAuthStatus>("disconnect_youtube"),
};
