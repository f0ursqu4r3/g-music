import type { MediaItem } from "@/api";
import type { LibraryArtist } from "./types";

interface ArtistAccumulator {
  albums: Set<string>;
  durationMs: number;
  name: string;
  trackCount: number;
  videoId: string;
}

export function buildLibraryArtists(
  tracks: Iterable<MediaItem>,
): LibraryArtist[] {
  const artists = new Map<string, ArtistAccumulator>();

  for (const track of tracks) {
    const artist = artists.get(track.artist) ?? {
      albums: new Set<string>(),
      durationMs: 0,
      name: track.artist,
      trackCount: 0,
      videoId: track.id,
    };
    const album = track.album?.trim();

    artist.durationMs += track.durationMs;
    artist.trackCount += 1;
    if (album) {
      artist.albums.add(album);
    }
    artists.set(track.artist, artist);
  }

  return [...artists.values()].map((artist) => ({
    albumCount: artist.albums.size,
    detail: `${artist.trackCount} ${artist.trackCount === 1 ? "song" : "songs"}`,
    durationMs: artist.durationMs,
    name: artist.name,
    trackCount: artist.trackCount,
    videoId: artist.videoId,
  }));
}

export function groupItems<T>(
  items: Iterable<T>,
  getLabel: (item: T) => string,
): Array<{ items: T[]; label: string }> {
  const groups = new Map<string, { items: T[]; label: string }>();

  for (const item of items) {
    const label = getLabel(item);
    const group = groups.get(label) ?? { items: [], label };

    group.items.push(item);
    groups.set(label, group);
  }

  return [...groups.values()];
}
