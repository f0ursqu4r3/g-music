import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";

import type { MediaItem, PlaybackSnapshot } from "@/api";
import { Slider } from "@/components/ui/slider";
import LibraryWindow from "../LibraryWindow.vue";

const importedTracks: MediaItem[] = [
  {
    album: "API Sessions",
    artist: "Google for Developers",
    durationMs: 238_000,
    id: "M7lc1UVf-VE",
    title: "YouTube Developers Live",
  },
  {
    album: "Creator Music",
    artist: "YouTube Creators",
    durationMs: 207_000,
    id: "BaW_jenozKc",
    title: "Creator Studio Session",
  },
];

const snapshot: PlaybackSnapshot = {
  status: "paused",
  currentItem: importedTracks[0],
  positionMs: 0,
  queue: importedTracks,
  volumePercent: 64,
};

describe("LibraryWindow", () => {
  it("shows playback command errors in the library window", () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        errorMessage: "the audio player failed",
        isUpdating: false,
        snapshot,
      },
    });

    expect(wrapper.get('[role="alert"]').text()).toBe(
      "the audio player failed",
    );
  });

  it("keeps tracks, albums, and artists in the library window", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-collection="albums"]').trigger("click");
    expect(wrapper.get("h1").text()).toBe("Albums");
    expect(wrapper.findAll(".album-tile")).toHaveLength(2);
    expect(wrapper.text()).toContain("API Sessions");

    await wrapper.get('[data-collection="artists"]').trigger("click");
    expect(wrapper.get("h1").text()).toBe("Artists");
    expect(wrapper.findAll(".artist-tile")).toHaveLength(2);
    expect(wrapper.text()).toContain("Google for Developers");
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
    expect(wrapper.findAll("tbody tr")).toHaveLength(2);
    expect(wrapper.findAll(".track-row-artwork")).toHaveLength(0);
    expect(wrapper.get(".track-playing-indicator")).toBeDefined();
    expect(wrapper.findAll('[aria-label^="Favorite "]')).toHaveLength(2);
    expect(wrapper.text()).toContain("YouTube Developers Live");
    expect(wrapper.text()).toContain("Google for Developers");
    expect(wrapper.text()).toContain("API Sessions");
    expect(wrapper.text()).toContain("3:58");
    expect(wrapper.text()).not.toContain("Night Drive over the City");
  });

  it("truncates track metadata within a fixed-layout table", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    expect(wrapper.get("table").classes()).toContain("table-fixed");
    for (const selector of [".track-title", ".track-artist", ".track-album"]) {
      const cellText = wrapper.get(selector);
      expect(cellText.classes()).toContain("overflow-hidden");
      expect(cellText.classes()).toContain("text-ellipsis");
      expect(cellText.classes()).toContain("whitespace-nowrap");
    }
  });

  it("provides keyboard-resizable track columns", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });
    const titleColumn = wrapper.get('col[data-column="title"]');
    const initialWidth = titleColumn.attributes("style");
    const titleResizeHandle = wrapper.get(
      '[role="separator"][aria-label="Resize Title column"]',
    );
    expect(wrapper.findAll('[role="separator"]')).toHaveLength(4);

    await titleResizeHandle.trigger("keydown", { key: "ArrowRight" });

    expect(titleColumn.attributes("style")).not.toBe(initialWidth);
  });

  it("resizes track columns by dragging a header boundary", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });
    const table = wrapper.get("table");
    vi.spyOn(table.element, "getBoundingClientRect").mockReturnValue({
      width: 1_000,
    } as DOMRect);
    const titleColumn = wrapper.get('col[data-column="title"]');
    const initialWidth = titleColumn.attributes("style");

    await wrapper
      .get('[aria-label="Resize Title column"]')
      .trigger("mousedown", { button: 0, clientX: 300 });
    window.dispatchEvent(new MouseEvent("mousemove", { clientX: 350 }));
    window.dispatchEvent(new MouseEvent("mouseup"));
    await wrapper.vm.$nextTick();

    expect(titleColumn.attributes("style")).not.toBe(initialWidth);
  });

  it("selects a track for playback from its table row", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("click");

    expect(wrapper.emitted("playTrack")).toEqual([["BaW_jenozKc"]]);
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

    expect(wrapper.findAll('input[type="range"]')).toHaveLength(0);
    expect(
      wrapper.get('[data-slot="slider"][aria-label="Track progress"]'),
    ).toBeDefined();
    const volume = wrapper
      .findAllComponents(Slider)
      .find((slider) => slider.attributes("aria-label") === "Volume");
    expect(volume).toBeDefined();
    volume!.vm.$emit("valueCommit", [72]);
    expect(wrapper.emitted("setVolume")).toEqual([[72]]);
  });

  it("toggles track favorites from the table", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const favorite = wrapper.get(
      'button[aria-label="Favorite Creator Studio Session"]',
    );
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

  it("opens Import Music from the plus button beside Library", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const heading = wrapper.get('[data-library-heading="library"]');
    const button = heading.get('button[aria-label="Import music"]');

    expect(heading.text()).toContain("Library");
    await button.trigger("click");
    expect(wrapper.emitted("openImport")).toEqual([[]]);
  });
});
