import { describe, expect, it, vi } from "vitest";

import type { PlaybackSnapshot } from "@/api";

import { usePlayback } from "./usePlayback";

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
    };
    const playback = usePlayback(client);

    await playback.refresh();
    await playback.toggle();

    expect(client.inspect).toHaveBeenCalledOnce();
    expect(client.play).toHaveBeenCalledOnce();
    expect(playback.snapshot.value).toEqual(playing);
  });
});
