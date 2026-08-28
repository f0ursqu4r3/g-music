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
      importYouTubeUrl: vi.fn().mockResolvedValue(playing),
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
      importYouTubeUrl: vi.fn(),
    };
    const playback = usePlayback(client);

    const synchronization = playback.sync();

    expect(playback.isUpdating.value).toBe(false);
    await synchronization;
    expect(playback.snapshot.value?.positionMs).toBe(2_000);
  });

  it("imports a pasted YouTube video or playlist URL", async () => {
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
      importYouTubeUrl: vi.fn().mockResolvedValue(playing),
    };
    const playback = usePlayback(client);

    await playback.importYouTubeUrl(
      "https://www.youtube.com/playlist?list=PL-example",
    );

    expect(client.importYouTubeUrl).toHaveBeenCalledWith(
      "https://www.youtube.com/playlist?list=PL-example",
    );
    expect(playback.snapshot.value).toEqual(playing);
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
      importYouTubeUrl: vi.fn(),
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
      importYouTubeUrl: vi.fn().mockRejectedValue({
        code: "youtube_metadata_failed",
        message: "could not resolve YouTube metadata",
      }),
    };
    const playback = usePlayback(client);

    await playback.importYouTubeUrl("https://youtu.be/wEsuJoBKAvA");

    expect(playback.errorMessage.value).toBe(
      "could not resolve YouTube metadata",
    );
  });
});
