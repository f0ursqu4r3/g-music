import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { PlaybackSnapshot } from "@/api";
import LibraryWindow from "./LibraryWindow.vue";

const snapshot: PlaybackSnapshot = {
  status: "paused",
  currentItem: {
    artist: "Northward",
    durationMs: 212_000,
    id: "night-shift",
    title: "Night Shift",
  },
  positionMs: 0,
  queue: [],
  volumePercent: 64,
};

describe("LibraryWindow", () => {
  it("keeps tracks, albums, and artists in the library window", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-collection="albums"]').trigger("click");
    expect(wrapper.get("h1").text()).toBe("Albums");
    expect(wrapper.findAll(".album-tile")).not.toHaveLength(0);

    await wrapper.get('[data-collection="artists"]').trigger("click");
    expect(wrapper.get("h1").text()).toBe("Artists");
    expect(wrapper.findAll(".artist-tile")).not.toHaveLength(0);
  });

  it("provides a native drag strip without a visible application header", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    expect(wrapper.find(".application-header").exists()).toBe(false);
    expect(wrapper.get(".application-drag-region").attributes()).toHaveProperty(
      "data-tauri-drag-region",
    );
  });

  it("uses one continuous glass surface for the library shell", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    expect(wrapper.get("aside").classes()).not.toContain(
      "bg-[var(--glass-sidebar)]",
    );
    expect(wrapper.get(".library-content").classes()).not.toContain(
      "bg-[oklch(0.12_0.012_248/0.22)]",
    );
    expect(wrapper.get("footer").classes()).not.toContain(
      "bg-[var(--glass-sidebar)]",
    );
  });
});
