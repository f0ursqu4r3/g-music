import type { MediaItem } from "@/api";

export const LIBRARY_TRACK_IDS_MIME_TYPE = "application/x-gmusic-track-ids";
export const LIBRARY_TRACK_IDS_TEXT_PREFIX = "gmusic-track-ids:";

export type LibraryCollection = "tracks" | "albums" | "artists";
export type LibraryDisplayMode = "grid" | "list";
export type LibrarySortOption =
  | "album-asc"
  | "album-desc"
  | "album-count-asc"
  | "album-count-desc"
  | "artist-asc"
  | "artist-desc"
  | "duration-asc"
  | "duration-desc"
  | "track-count-asc"
  | "track-count-desc"
  | "title-asc"
  | "title-desc";
export type LibraryGroupOption = "album" | "artist" | "none";

export interface LibraryAlbum {
  key: string;
  artist: string;
  durationMs: number;
  trackCount: number;
  title: string;
  videoId: string;
}

export interface LibraryArtist {
  detail: string;
  durationMs: number;
  name: string;
  trackCount: number;
  albumCount: number;
  videoId: string;
}

export interface TrackFilter {
  label: string;
  type: "album" | "artist";
  value: string;
}

export interface TrackSelectionModifiers {
  additive: boolean;
  range: boolean;
}

export interface TrackGroup {
  items: MediaItem[];
  label: string;
}

export interface AlbumGroup {
  items: LibraryAlbum[];
  label: string;
}

export interface ArtistGroup {
  items: LibraryArtist[];
  label: string;
}
