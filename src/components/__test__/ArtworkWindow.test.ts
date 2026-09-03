import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { PlaybackSnapshot } from "@/api";
import { Slider } from "@/components/ui/slider";

import AutoScrollText from "../AutoScrollText.vue";
import ArtworkWindow from "../ArtworkWindow.vue";
import YouTubeArtwork from "../YouTubeArtwork.vue";
import { dragSlider } from "./slider-interaction";

const snapshot: PlaybackSnapshot = {
  status: "paused",
  currentItem: {
    id: "M7lc1UVf-VE",
    title: "Night Drive",
    artist: "Chromatic Skies",
    durationMs: 238_000,
  },
  positionMs: 57_000,
  volumePercent: 72,
  queue: [],
};

describe("ArtworkWindow", () => {
  it("fills the frame with artwork behind bottom-overlay controls", () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false },
    });

    expect(wrapper.get("main").classes()).toEqual(
      expect.arrayContaining(["relative", "h-screen", "overflow-hidden"]),
    );
    expect(wrapper.get(".artwork-cover").classes()).toEqual(
      expect.arrayContaining(["absolute", "inset-0"]),
    );
    expect(wrapper.getComponent(YouTubeArtwork).props("videoId")).toBe(
      "M7lc1UVf-VE",
    );
    expect(
      wrapper.get(".artwork-drag-region").attributes("data-tauri-drag-region"),
    ).toBe("");
    expect(wrapper.get("main").classes()).toEqual(
      expect.arrayContaining(["flex", "flex-col"]),
    );
    expect(wrapper.get(".artwork-drag-region").classes()).toEqual(
      expect.arrayContaining(["min-h-0", "flex-1"]),
    );

    const overlay = wrapper.get(".artwork-controls");
    expect(overlay.classes()).toEqual(
      expect.arrayContaining([
        "artwork-information-gradient",
        "relative",
        "shrink-0",
      ]),
    );
    expect(wrapper.find(".artwork-scrim").exists()).toBe(false);
    expect(wrapper.findAll('input[type="range"]')).toHaveLength(0);
    const progress = wrapper.getComponent(Slider);
    expect(progress.attributes("aria-label")).toBe("Track progress");
    expect(progress.props("disabled")).toBe(false);
    expect(wrapper.text()).not.toContain("G MUSIC");
  });

  it("emits transport actions from the overlaid controls", async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false },
    });

    await wrapper.get('[aria-label="Previous track"]').trigger("click");
    await wrapper.get('[aria-label="Play"]').trigger("click");
    await wrapper.get('[aria-label="Next track"]').trigger("click");

    expect(wrapper.emitted("previous")).toHaveLength(1);
    expect(wrapper.emitted("toggle")).toHaveLength(1);
    expect(wrapper.emitted("next")).toHaveLength(1);
  });

  it("exposes current-track favorite actions from the artwork surface", async () => {
    const wrapper = mount(ArtworkWindow, {
      attachTo: document.body,
      props: { snapshot, isUpdating: false },
    });

    await wrapper.get('[aria-label="Favorite track"]').trigger("click");
    expect(wrapper.emitted("toggleFavorite")).toEqual([["M7lc1UVf-VE"]]);

    await wrapper.get(".artwork-cover").trigger("contextmenu");
    expect(
      document.body.querySelector("[data-artwork-context-menu]")?.textContent,
    ).toContain("Add to Favorites");
    wrapper.unmount();
  });

  it("commits pointer drags from the progress scrubber", async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false },
    });
    const progress = wrapper.get(
      '[data-slot="slider"][aria-label="Track progress"]',
    );

    await dragSlider(progress, 50);

    expect(wrapper.emitted("seek")).toEqual([[119_000]]);
  });

  it("slides controls out of view when the native window loses focus", async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false, isWindowFocused: true },
    });
    const metadata = wrapper.get(".artwork-metadata");
    const controls = wrapper.get(".artwork-playback-controls");

    expect(controls.attributes("data-window-focused")).toBe("true");

    await wrapper.setProps({ isWindowFocused: false });

    expect(controls.attributes("data-window-focused")).toBe("false");
    expect(controls.attributes("aria-hidden")).toBe("true");
    expect(controls.attributes()).toHaveProperty("inert");
    expect(metadata.text()).toContain("Night Drive");
    expect(metadata.text()).toContain("Chromatic Skies");
    expect(wrapper.findAllComponents(AutoScrollText)).toHaveLength(2);
    expect(wrapper.findAllComponents(AutoScrollText)[0].props()).toMatchObject({
      as: "h1",
      speed: "slow",
      text: "Night Drive",
    });
    expect(wrapper.findAllComponents(AutoScrollText)[1].props()).toMatchObject({
      as: "p",
      speed: "medium",
      text: "Chromatic Skies",
    });
    expect(metadata.attributes("aria-hidden")).toBeUndefined();
    expect(metadata.attributes()).not.toHaveProperty("inert");
    expect(controls.classes()).toEqual(
      expect.arrayContaining([
        "max-h-32",
        "overflow-hidden",
        "transition-[max-height,margin,opacity,transform]",
        "data-[window-focused=false]:max-h-0",
        "data-[window-focused=false]:mt-0",
        "data-[window-focused=false]:translate-y-full",
        "data-[window-focused=false]:opacity-0",
        "motion-reduce:transition-none",
      ]),
    );
    expect(controls.classes()).not.toContain("h-0.5");
  });

  it("shows track progress along the panel bottom while controls are hidden", async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false, isWindowFocused: true },
    });

    expect(wrapper.find(".artwork-unfocused-progress").exists()).toBe(false);

    await wrapper.setProps({ isWindowFocused: false });

    const progress = wrapper.get(".artwork-unfocused-progress");
    expect(progress.element.tagName).toBe("PROGRESS");
    expect(progress.attributes("aria-label")).toBe("Track progress");
    expect(progress.attributes("value")).toBe("57000");
    expect(progress.attributes("max")).toBe("238000");
    expect(progress.classes()).toEqual(
      expect.arrayContaining(["absolute", "right-0", "bottom-0", "left-0"]),
    );
  });
});
