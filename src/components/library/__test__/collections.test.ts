import { describe, expect, it } from "vitest";

import type { MediaItem } from "@/api";
import { buildLibraryArtists, groupItems } from "../collections";

const tracks: MediaItem[] = [
  {
    album: "Signal",
    artist: "Alpha",
    durationMs: 120_000,
    id: "alpha-1",
    title: "First",
  },
  {
    album: "Signal",
    artist: "Alpha",
    durationMs: 180_000,
    id: "alpha-2",
    title: "Second",
  },
  {
    album: "Orbit",
    artist: "Alpha",
    durationMs: 240_000,
    id: "alpha-3",
    title: "Third",
  },
  {
    album: null,
    artist: "Beta",
    durationMs: 90_000,
    id: "beta-1",
    title: "Fourth",
  },
];

describe("library collection builders", () => {
  it("aggregates each artist without rescanning the track collection", () => {
    expect(buildLibraryArtists(tracks)).toEqual([
      {
        albumCount: 2,
        detail: "3 songs",
        durationMs: 540_000,
        name: "Alpha",
        trackCount: 3,
        videoId: "alpha-1",
      },
      {
        albumCount: 0,
        detail: "1 song",
        durationMs: 90_000,
        name: "Beta",
        trackCount: 1,
        videoId: "beta-1",
      },
    ]);
  });

  it("groups values in input order without copying a group for each item", () => {
    expect(groupItems(tracks, (track) => track.artist)).toEqual([
      { items: tracks.slice(0, 3), label: "Alpha" },
      { items: [tracks[3]], label: "Beta" },
    ]);
  });
});
