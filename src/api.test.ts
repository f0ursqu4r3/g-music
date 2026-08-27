import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { playbackApi, type PlaybackSnapshot } from "./api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const snapshot: PlaybackSnapshot = {
  status: "paused",
  currentItem: null,
  positionMs: 0,
  volumePercent: 72,
  queue: [],
};

describe("playbackApi", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("requests the playback snapshot from the inspect command", async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot);

    await expect(playbackApi.inspect()).resolves.toEqual(snapshot);

    expect(invoke).toHaveBeenCalledWith("inspect_playback");
  });
});
