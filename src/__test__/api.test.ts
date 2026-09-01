import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";

import {
  playbackApi,
  type PlaybackSnapshot,
  windowApi,
  youtubeAuthApi,
} from "../api";

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

  it("starts many YouTube sources through the background import service", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const urls = [
      "https://www.youtube.com/playlist?list=PL-example",
      "https://www.youtube.com/@artist/videos",
    ];

    await expect(playbackApi.importYouTubeUrls(urls)).resolves.toBeUndefined();

    expect(invoke).toHaveBeenCalledWith("import_youtube_urls", { urls });
  });

  it("uses a payload-free response for live volume updates", () => {
    expectTypeOf(playbackApi.setVolume).returns.toEqualTypeOf<Promise<void>>();
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

describe("windowApi", () => {
  it("opens the dedicated Import Music window", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await expect(windowApi.showImport()).resolves.toBeUndefined();

    expect(invoke).toHaveBeenCalledWith("show_import_window");
  });
});
