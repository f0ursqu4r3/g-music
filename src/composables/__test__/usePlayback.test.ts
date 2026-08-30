import { describe, expect, it, vi } from "vitest";

import type { PlaybackSnapshot } from "@/api";

import { usePlayback } from "../usePlayback";

const paused: PlaybackSnapshot = {
  status: "paused",
  currentItem: null,
  positionMs: 0,
  volumePercent: 70,
  queue: [],
};

const playing: PlaybackSnapshot = { ...paused, status: "playing" };

describe("usePlayback", () => {
  it("loads a snapshot then uses play for a paused track", async () => {
    const client = {
      inspect: vi.fn().mockResolvedValue(paused),
      play: vi.fn().mockResolvedValue(playing),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn().mockResolvedValue(playing),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    await playback.toggle();

    expect(client.inspect).toHaveBeenCalledOnce();
    expect(client.play).toHaveBeenCalledOnce();
    expect(playback.snapshot.value).toEqual(playing);
  });

  it("synchronizes advancing playback without entering the updating state", async () => {
    const advanced = { ...playing, positionMs: 2_000 };
    const client = {
      inspect: vi.fn().mockResolvedValue(advanced),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn(),
    };
    const playback = usePlayback(client);

    const synchronization = playback.sync();

    expect(playback.isUpdating.value).toBe(false);
    await synchronization;
    expect(playback.snapshot.value?.positionMs).toBe(2_000);
  });

  it("keeps library data stable while synchronizing transport state", async () => {
    const library = {
      tracks: [
        {
          artist: "YouTube Creators",
          durationMs: 207_000,
          id: "BaW_jenozKc",
          title: "Creator Studio Session",
        },
      ],
    };
    const initialTransport = {
      currentItem: library.tracks[0],
      positionMs: 0,
      status: "playing" as const,
      volumePercent: 70,
    };
    const advancedTransport = { ...initialTransport, positionMs: 2_000 };
    const client = {
      importYouTubeUrls: vi.fn(),
      inspect: vi.fn(),
      inspectLibrary: vi.fn().mockResolvedValue(library),
      inspectTransport: vi
        .fn()
        .mockResolvedValueOnce(initialTransport)
        .mockResolvedValueOnce(advancedTransport),
      moveQueueItem: vi.fn(),
      next: vi.fn(),
      pause: vi.fn(),
      play: vi.fn(),
      playTrack: vi.fn(),
      previous: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    const loadedLibrary = playback.library.value;
    await playback.sync();

    expect(playback.library.value).toBe(loadedLibrary);
    expect(playback.transport.value?.positionMs).toBe(2_000);
    expect(client.inspectLibrary).toHaveBeenCalledOnce();
    expect(client.inspectTransport).toHaveBeenCalledTimes(2);
  });

  it("starts import work without blocking playback commands", async () => {
    const client = {
      inspect: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn().mockResolvedValue(undefined),
    };
    const playback = usePlayback(client);
    const urls = [
      "https://www.youtube.com/playlist?list=PL-example",
      "https://www.youtube.com/@artist/videos",
    ];

    await playback.importYouTubeUrls(urls);

    expect(client.importYouTubeUrls).toHaveBeenCalledWith(urls);
    expect(playback.isUpdating.value).toBe(false);
    expect(playback.isImporting.value).toBe(true);

    playback.updateImportProgress({
      completedSources: 2,
      importedTracks: 7,
      message: "Imported 7 track(s) into the library.",
      phase: "completed",
      runId: 3,
      skippedMemberOnly: 0,
      totalSources: 2,
    });

    expect(playback.isImporting.value).toBe(false);
    expect(playback.importProgress.value?.importedTracks).toBe(7);
  });

  it("plays a selected library track", async () => {
    const selected = {
      ...playing,
      currentItem: {
        artist: "YouTube Creators",
        durationMs: 207_000,
        id: "BaW_jenozKc",
        title: "Creator Studio Session",
      },
    };
    const client = {
      inspect: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn().mockResolvedValue(selected),
      importYouTubeUrls: vi.fn(),
    };
    const playback = usePlayback(client);

    await playback.playTrack("BaW_jenozKc");

    expect(client.playTrack).toHaveBeenCalledWith("BaW_jenozKc");
    expect(playback.snapshot.value).toEqual(selected);
  });

  it("preserves a structured Tauri command error message", async () => {
    const client = {
      inspect: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn().mockRejectedValue({
        code: "youtube_metadata_failed",
        message: "could not resolve YouTube metadata",
      }),
    };
    const playback = usePlayback(client);

    await playback.importYouTubeUrls(["https://youtu.be/wEsuJoBKAvA"]);

    expect(playback.errorMessage.value).toBe(
      "could not resolve YouTube metadata",
    );
  });
});
