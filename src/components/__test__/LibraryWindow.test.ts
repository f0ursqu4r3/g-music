import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { PlaybackSnapshot } from "@/api";
import LibraryWindow from "../LibraryWindow.vue";

const snapshot: PlaybackSnapshot = {
  status: "paused",
  currentItem: {
    artist: "Chromatic Skies",
    durationMs: 238_000,
    id: "night-drive",
    title: "Night Drive",
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

  it("uses the compact reference-style library header and track table", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    expect(wrapper.text()).not.toContain("Your library");
    expect(wrapper.text()).not.toContain(
      "Artwork and queue are in the Window menu",
    );
    expect(wrapper.get('[aria-label="Library view options"]')).toBeDefined();
    expect(
      wrapper.findAll('[aria-label="Library view options"] button'),
    ).toHaveLength(3);
    expect(wrapper.findAll("thead th")).toHaveLength(5);
    expect(wrapper.findAll("tbody tr")).toHaveLength(8);
    expect(wrapper.findAll(".track-row-artwork")).toHaveLength(0);
    expect(wrapper.get(".track-playing-indicator")).toBeDefined();
    expect(wrapper.findAll('[aria-label^="Favorite "]')).toHaveLength(8);
  });

  it("provides the complete reference-style playback strip", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    for (const label of [
      "Shuffle",
      "Previous track",
      "Play",
      "Next track",
      "Repeat",
    ]) {
      expect(wrapper.get(`button[aria-label="${label}"]`)).toBeDefined();
    }

    expect(wrapper.get('input[aria-label="Track progress"]')).toBeDefined();
    const volume = wrapper.get('input[aria-label="Volume"]');
    await volume.setValue("72");
    expect(wrapper.emitted("setVolume")).toEqual([[72]]);
  });

  it("toggles track favorites from the table", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const favorite = wrapper.get('button[aria-label="Favorite The Current"]');
    expect(favorite.attributes("aria-pressed")).toBe("false");

    await favorite.trigger("click");

    expect(favorite.attributes("aria-pressed")).toBe("true");
  });

  it("fills the sidebar with library and playlist shortcuts", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    expect(wrapper.get('[data-library-destination="playlists"]').text()).toBe(
      "Playlists",
    );
    expect(wrapper.findAll('nav[aria-label="Playlists"] a')).toHaveLength(5);
  });
});
