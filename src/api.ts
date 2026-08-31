import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface MediaItem {
  id: string;
  provider?: string;
  sourceUrl?: string | null;
  title: string;
  artist: string;
  album?: string | null;
  albumArtist?: string | null;
  trackNumber?: number | null;
  discNumber?: number | null;
  releaseDate?: string | null;
  uploadDate?: string | null;
  description?: string | null;
  channel?: string | null;
  channelId?: string | null;
  uploader?: string | null;
  uploaderId?: string | null;
  thumbnailUrl?: string | null;
  label?: string | null;
  genres?: string[];
  categories?: string[];
  tags?: string[];
  language?: string | null;
  availability?: string | null;
  isLive?: boolean;
  viewCount?: number | null;
  likeCount?: number | null;
  durationMs: number;
  metadataDirty?: boolean;
}

export type PlaybackStatus = "paused" | "playing";

export interface PlaybackSnapshot {
  status: PlaybackStatus;
  currentItem: MediaItem | null;
  positionMs: number;
  volumePercent: number;
  queue: MediaItem[];
}

export interface PlaybackTransport {
  status: PlaybackStatus;
  currentItem: MediaItem | null;
  positionMs: number;
  volumePercent: number;
}

export interface LibrarySnapshot {
  tracks: MediaItem[];
}

export type ImportProgressPhase =
  "started" | "resolving" | "merging" | "completed" | "failed";

export interface ImportProgress {
  completedSources: number;
  importedTracks: number;
  message: string;
  phase: ImportProgressPhase;
  runId: number;
  skippedMemberOnly: number;
  totalSources: number;
}

export type MetadataRefreshState =
  "queued" | "refreshing" | "completed" | "failed";

export interface MetadataRefreshJob {
  message: string;
  state: MetadataRefreshState;
  trackId: string;
  title: string;
}

export interface MetadataRefreshSnapshot {
  completedTracks: number;
  jobs: MetadataRefreshJob[];
  totalTracks: number;
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
  inspectLibrary: (): Promise<LibrarySnapshot> =>
    invoke<LibrarySnapshot>("inspect_library"),
  inspectTransport: (): Promise<PlaybackTransport> =>
    invoke<PlaybackTransport>("inspect_playback_transport"),
  inspectMetadataRefreshes: (): Promise<MetadataRefreshSnapshot> =>
    invoke<MetadataRefreshSnapshot>("inspect_metadata_refreshes"),
  importYouTubeUrls: (urls: string[]): Promise<void> =>
    invoke<void>("import_youtube_urls", { urls }),
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

export const artworkApi = {
  resolveYouTube: async (videoId: string): Promise<string | null> => {
    const localPath = await invoke<string | null>("resolve_youtube_artwork", {
      videoId,
    });
    return localPath ? convertFileSrc(localPath) : null;
  },
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
