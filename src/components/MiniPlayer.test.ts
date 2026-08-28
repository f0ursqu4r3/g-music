import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { PlaybackSnapshot } from "@/api";

import MiniPlayer from "./MiniPlayer.vue";

const snapshot: PlaybackSnapshot = {
  status: "paused",
  currentItem: {
    id: "track-1",
    title: "Aerials",
    artist: "System of a Down",
    durationMs: 235_000,
  },
  positionMs: 63_000,
  volumePercent: 72,
  queue: [],
};

describe("MiniPlayer", () => {
  it("shows the current track and emits playback controls", async () => {
    const wrapper = mount(MiniPlayer, {
      props: { snapshot, isUpdating: false },
    });

    expect(wrapper.text()).toContain("Aerials");
    expect(wrapper.text()).toContain("System of a Down");
    expect(wrapper.get('button[aria-label="Play"]')).toBeDefined();
    expect(wrapper.get(".mini-drag-handle").attributes()).toHaveProperty(
      "data-tauri-drag-region",
    );

    await wrapper.get('button[aria-label="Play"]').trigger("click");
    await wrapper.get('button[aria-label="Next track"]').trigger("click");

    expect(wrapper.emitted("toggle")).toHaveLength(1);
    expect(wrapper.emitted("next")).toHaveLength(1);
  });
});
