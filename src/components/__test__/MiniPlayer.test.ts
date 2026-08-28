import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { PlaybackSnapshot } from "@/api";
import { Slider } from "@/components/ui/slider";

import MiniPlayer from "../MiniPlayer.vue";
import YouTubeArtwork from "../YouTubeArtwork.vue";

const snapshot: PlaybackSnapshot = {
  status: "paused",
  currentItem: {
    id: "M7lc1UVf-VE",
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
    expect(wrapper.getComponent(YouTubeArtwork).props("videoId")).toBe(
      "M7lc1UVf-VE",
    );

    await wrapper.get('button[aria-label="Play"]').trigger("click");
    await wrapper.get('button[aria-label="Next track"]').trigger("click");

    expect(wrapper.emitted("toggle")).toHaveLength(1);
    expect(wrapper.emitted("next")).toHaveLength(1);
  });

  it("uses shadcn sliders for seeking and volume commits", () => {
    const wrapper = mount(MiniPlayer, {
      props: { snapshot, isUpdating: false },
    });

    expect(wrapper.findAll('input[type="range"]')).toHaveLength(0);

    const sliders = wrapper.findAllComponents(Slider);
    const volume = sliders.find(
      (slider) => slider.attributes("aria-label") === "Volume",
    );
    const progress = sliders.find(
      (slider) => slider.attributes("aria-label") === "Track progress",
    );

    expect(volume).toBeDefined();
    expect(progress).toBeDefined();

    for (const thumb of wrapper.findAll('[data-slot="slider-thumb"]')) {
      expect(thumb.classes()).toEqual(
        expect.arrayContaining([
          "opacity-0",
          "group-hover/slider:opacity-100",
          "group-focus-within/slider:opacity-100",
        ]),
      );
    }

    volume!.vm.$emit("valueCommit", [61]);
    progress!.vm.$emit("valueCommit", [96_000]);

    expect(wrapper.emitted("setVolume")).toEqual([[61]]);
    expect(wrapper.emitted("seek")).toEqual([[96_000]]);
  });
});
