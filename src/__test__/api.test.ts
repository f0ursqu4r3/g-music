import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { playbackApi, type PlaybackSnapshot, youtubeAuthApi } from "../api";

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

  it("imports a YouTube video or playlist through the playback service", async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot);

    await expect(
      playbackApi.importYouTubeUrl(
        "https://www.youtube.com/playlist?list=PL-example",
      ),
    ).resolves.toEqual(snapshot);

    expect(invoke).toHaveBeenCalledWith("import_youtube_url", {
      url: "https://www.youtube.com/playlist?list=PL-example",
    });
  });

  it("plays a selected library track through the playback service", async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot);

    await expect(playbackApi.playTrack("M7lc1UVf-VE")).resolves.toEqual(
      snapshot,
    );

    expect(invoke).toHaveBeenCalledWith("play_track", {
      id: "M7lc1UVf-VE",
    });
  });
});

describe("youtubeAuthApi", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("uses isolated commands for the YouTube account session", async () => {
    vi.mocked(invoke).mockResolvedValue({ connected: false });

    await youtubeAuthApi.inspect();
    await youtubeAuthApi.openLogin();
    await youtubeAuthApi.saveSession();
    await youtubeAuthApi.disconnect();

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["inspect_youtube_auth"],
      ["open_youtube_login"],
      ["save_youtube_session"],
      ["disconnect_youtube"],
    ]);
  });
});
