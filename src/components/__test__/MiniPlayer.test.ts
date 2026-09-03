import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { PlaybackSnapshot } from "@/api";
import { Slider } from "@/components/ui/slider";

import MiniPlayer from "../MiniPlayer.vue";
import YouTubeArtwork from "../YouTubeArtwork.vue";
import { dragSlider } from "./slider-interaction";

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
    expect(wrapper.get("[data-tauri-drag-region]").attributes()).toHaveProperty(
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

  it("matches the main player playback capabilities and primary-control style", async () => {
    const wrapper = mount(MiniPlayer, {
      props: {
        snapshot: {
          ...snapshot,
          repeatMode: "all",
          shuffleEnabled: true,
        },
        isUpdating: false,
        favoriteTrackIds: ["M7lc1UVf-VE"],
      },
    });

    const shuffle = wrapper.get('button[aria-label="Disable shuffle"]');
    const repeat = wrapper.get('button[aria-label="Enable repeat one"]');
    const favorite = wrapper.get('button[aria-label="Favorite track"]');
    const play = wrapper.get('button[aria-label="Play"]');

    expect(shuffle.attributes("aria-pressed")).toBe("true");
    expect(repeat.attributes("aria-pressed")).toBe("true");
    expect(favorite.attributes("aria-pressed")).toBe("true");
    expect(play.classes()).toEqual(
      expect.arrayContaining(["size-10", "rounded-full", "bg-(--text)"]),
    );

    await shuffle.trigger("click");
    await repeat.trigger("click");
    await favorite.trigger("click");

    expect(wrapper.emitted("toggleShuffle")).toEqual([[]]);
    expect(wrapper.emitted("cycleRepeatMode")).toEqual([[]]);
    expect(wrapper.emitted("toggleFavorite")).toEqual([["M7lc1UVf-VE"]]);
  });

  it("uses shadcn sliders for seeking and live volume updates", () => {
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

    volume!.vm.$emit("update:modelValue", [61]);
    progress!.vm.$emit("valueCommit", [96_000]);

    expect(wrapper.emitted("setVolume")).toEqual([[61]]);
    expect(wrapper.emitted("seek")).toEqual([[96_000]]);
  });

  it("uses volume level icons and toggles mute", async () => {
    const wrapper = mount(MiniPlayer, {
      props: { snapshot, isUpdating: false },
    });

    expect(wrapper.get(".lucide-volume-2")).toBeDefined();
    expect(wrapper.get('button[aria-label="Mute volume"]')).toBeDefined();
    await wrapper.get('button[aria-label="Mute volume"]').trigger("click");
    expect(wrapper.emitted("toggleMute")).toEqual([[]]);

    await wrapper.setProps({ snapshot: { ...snapshot, volumePercent: 0 } });

    expect(wrapper.get('button[aria-label="Unmute volume"]')).toBeDefined();
    await wrapper.get('button[aria-label="Unmute volume"]').trigger("click");
    expect(wrapper.emitted("toggleMute")).toEqual([[], []]);

    await wrapper.setProps({ snapshot: { ...snapshot, volumePercent: 50 } });
    expect(wrapper.get(".lucide-volume-1")).toBeDefined();
    await wrapper.setProps({ snapshot: { ...snapshot, volumePercent: 20 } });
    expect(wrapper.get(".lucide-volume")).toBeDefined();
  });

  it("commits pointer drags from the progress and volume scrubbers", async () => {
    const wrapper = mount(MiniPlayer, {
      props: { snapshot, isUpdating: false },
    });
    const volume = wrapper.get('[data-slot="slider"][aria-label="Volume"]');
    const progress = wrapper.get(
      '[data-slot="slider"][aria-label="Track progress"]',
    );

    await dragSlider(volume, 25);
    await dragSlider(progress, 50);

    expect(wrapper.emitted("setVolume")).toEqual([[25]]);
    expect(wrapper.emitted("seek")).toEqual([[118_000]]);
  });
});
