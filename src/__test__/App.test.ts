import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import App from "../App.vue";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(),
  LogicalSize: vi.fn(),
}));

vi.mock("@/composables/usePlayback", () => ({
  usePlayback: () => ({
    snapshot: {
      value: {
        status: "paused",
        currentItem: {
          id: "night-drive",
          title: "Night Drive",
          artist: "Chromatic Skies",
          durationMs: 238_000,
        },
        positionMs: 57_000,
        volumePercent: 72,
        queue: [],
      },
    },
    isUpdating: { value: false },
    errorMessage: { value: "" },
    refresh: vi.fn(),
    toggle: vi.fn(),
    previous: vi.fn(),
    next: vi.fn(),
    seek: vi.fn(),
    setVolume: vi.fn(),
    moveQueueItem: vi.fn(),
  }),
}));

describe("application landmarks", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", "/?view=library");
    window.localStorage.clear();
  });

  afterEach(() => {
    document.documentElement.removeAttribute("data-theme");
  });

  it("renders exactly one main landmark for the active native window", async () => {
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.findAll("main")).toHaveLength(1);
    expect(wrapper.get("main").attributes("aria-label")).toBe("Music library");
  });
});
