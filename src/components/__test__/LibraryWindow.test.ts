import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";

import type { MediaItem, PlaybackSnapshot, PlaybackTransport } from "@/api";
import { Slider } from "@/components/ui/slider";
import LibraryWindow from "../LibraryWindow.vue";
import { dragSlider } from "./slider-interaction";

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
const transport: PlaybackTransport = {
  currentItem: importedTracks[0],
  positionMs: 0,
  status: "paused",
  volumePercent: 64,
};

describe("LibraryWindow", () => {
  it("renders from stable library tracks and separate transport state", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, tracks: importedTracks, transport },
    });

    expect(wrapper.findAll("[data-track-id]")).toHaveLength(2);
    expect(wrapper.get('[aria-label="Track progress"]').text()).toContain(
      "0:00",
    );
  });

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

  it("keeps the selected collection when switching between list and grid", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('button[aria-label="Grid view"]').trigger("click");

    expect(wrapper.get("h1").text()).toBe("Tracks");
    expect(
      wrapper.get('button[aria-label="Grid view"]').attributes("aria-pressed"),
    ).toBe("true");
    expect(wrapper.find(".track-grid").exists()).toBe(true);

    await wrapper.get('[data-collection="albums"]').trigger("click");
    expect(wrapper.get("h1").text()).toBe("Albums");
    expect(wrapper.find(".album-tile").exists()).toBe(true);
    expect(
      wrapper.get('button[aria-label="Grid view"]').attributes("aria-pressed"),
    ).toBe("true");
  });

  it("sorts tracks in both list and grid views", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper
      .get('button[aria-label="More library options"]')
      .trigger("click");
    await wrapper.get('[data-sort="title-desc"]').trigger("click");
    expect(
      wrapper.get('[data-track-id="M7lc1UVf-VE"] .track-title').text(),
    ).toBe("YouTube Developers Live");

    await wrapper.get('button[aria-label="Grid view"]').trigger("click");
    expect(wrapper.get(".track-grid .track-tile").text()).toContain(
      "YouTube Developers Live",
    );
  });

  it("groups grid items without changing the list view", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper
      .get('button[aria-label="More library options"]')
      .trigger("click");
    await wrapper.get('[data-group="artist"]').trigger("click");
    expect(wrapper.findAll(".library-group")).toHaveLength(0);

    await wrapper.get('button[aria-label="Grid view"]').trigger("click");
    expect(wrapper.findAll(".library-group")).toHaveLength(2);
  });

  it("filters tracks after double-clicking an album or artist", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-collection="albums"]').trigger("click");
    await wrapper.get(".album-tile").trigger("dblclick");
    expect(wrapper.get("h1").text()).toBe("Tracks");
    expect(wrapper.get("[data-library-filter]").text()).toContain(
      "API Sessions",
    );
    expect(wrapper.findAll("[data-track-id]")).toHaveLength(1);

    await wrapper.get('[data-collection="artists"]').trigger("click");
    await wrapper.get(".artist-tile").trigger("dblclick");
    expect(wrapper.get("h1").text()).toBe("Tracks");
    expect(wrapper.get("[data-library-filter]").text()).toContain(
      "Google for Developers",
    );
    expect(wrapper.findAll("[data-track-id]")).toHaveLength(1);
  });

  it("shows metadata for the selected track, album, or artist", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("click");
    expect(wrapper.get('[data-library-info="track"]').text()).toContain(
      "Creator Studio Session",
    );

    await wrapper.get('[data-collection="albums"]').trigger("click");
    await wrapper.get(".album-tile").trigger("click");
    expect(wrapper.get('[data-library-info="album"]').text()).toContain(
      "API Sessions",
    );
  });

  it("exposes a grid item size slider in grid view", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    expect(wrapper.find('[aria-label="Grid item size"]').exists()).toBe(false);
    await wrapper.get('button[aria-label="Grid view"]').trigger("click");
    expect(wrapper.get('[aria-label="Grid item size"]')).toBeDefined();
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

  it("keeps the whole sidebar scrollable when its content overflows", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const sidebar = wrapper.get("[data-library-sidebar]");
    const playlists = wrapper.get("[data-library-playlists]");
    expect(sidebar.classes()).toContain("overflow-y-auto");
    expect(playlists.classes()).not.toContain("overflow-y-auto");
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
    expect(
      wrapper.findAll('[data-library-track-virtualizer] [role="row"]'),
    ).toHaveLength(2);
    expect(wrapper.findAll(".track-row-artwork")).toHaveLength(0);
    expect(wrapper.get(".track-playing-indicator")).toBeDefined();
    expect(
      wrapper
        .get("[data-library-track-list]")
        .findAll('[aria-label^="Favorite "]'),
    ).toHaveLength(2);
    expect(wrapper.text()).toContain("YouTube Developers Live");
    expect(wrapper.text()).toContain("Google for Developers");
    expect(wrapper.text()).toContain("API Sessions");
    expect(wrapper.text()).toContain("3:58");
    expect(wrapper.text()).not.toContain("Night Drive over the City");
  });

  it("shows active metadata work as a remaining counter after the library summary", () => {
    const activeRefreshes = {
      completedTracks: 4,
      jobs: [
        {
          message: "Fetching full YouTube metadata.",
          state: "refreshing" as const,
          title: importedTracks[0].title,
          trackId: importedTracks[0].id,
        },
      ],
      totalTracks: 9,
    };
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        metadataRefreshes: activeRefreshes,
        snapshot,
      },
    });

    const summary = wrapper.get("[data-library-summary]");
    const remaining = wrapper.get("[data-metadata-refresh-remaining]");
    expect(summary.text()).toContain("2 songs");
    expect(remaining.text()).toBe("5");
    expect(summary.element.nextElementSibling).toBe(remaining.element);

    const completedWrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        metadataRefreshes: {
          ...activeRefreshes,
          completedTracks: 9,
          jobs: [{ ...activeRefreshes.jobs[0], state: "completed" }],
        },
        snapshot,
      },
    });
    expect(
      completedWrapper.find("[data-metadata-refresh-remaining]").exists(),
    ).toBe(false);
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

  it("keeps track headings above a masked virtualized library", async () => {
    const tracks = Array.from({ length: 200 }, (_, index) => ({
      ...importedTracks[index % importedTracks.length]!,
      id: `track-${index}`,
      title: `Track ${index}`,
    }));
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: { ...snapshot, queue: tracks },
      },
    });
    const trackList = wrapper.get("[data-library-track-list]");

    expect(wrapper.get("[data-library-track-header]").classes()).toContain(
      "shrink-0",
    );
    expect(wrapper.get("thead").classes()).not.toContain("sticky");
    expect(wrapper.find("[data-library-track-header-fade]").exists()).toBe(
      false,
    );
    expect(trackList.classes()).toContain("library-track-scroll");
    expect(wrapper.findAll("tbody [data-track-id]").length).toBeLessThan(
      tracks.length,
    );

    trackList.element.scrollTop = 44 * 100;
    await trackList.trigger("scroll");

    expect(wrapper.find('[data-track-id="track-100"]').exists()).toBe(true);
    expect(
      wrapper.get('[data-track-id="track-100"]').attributes("data-index"),
    ).toBe("100");
    expect(wrapper.find('[data-track-id="track-0"]').exists()).toBe(false);
  });

  it("positions virtualized track rows outside a table body", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });
    const virtualizer = wrapper.get("[data-library-track-virtualizer]");

    expect(virtualizer.findAll('[role="row"]')).toHaveLength(2);
    expect(virtualizer.find("table").exists()).toBe(false);
  });

  it("resets virtual track position when the list remounts", async () => {
    const tracks = Array.from({ length: 200 }, (_, index) => ({
      ...importedTracks[index % importedTracks.length]!,
      id: `track-${index}`,
      title: `Track ${index}`,
    }));
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: { ...snapshot, queue: tracks },
      },
    });
    const trackList = wrapper.get("[data-library-track-list]");

    trackList.element.scrollTop = 44 * 100;
    await trackList.trigger("scroll");
    expect(wrapper.find('[data-track-id="track-100"]').exists()).toBe(true);

    await wrapper.get('button[aria-label="Grid view"]').trigger("click");
    await wrapper.get('button[aria-label="List view"]').trigger("click");

    expect(wrapper.find('[data-track-id="track-0"]').exists()).toBe(true);
    expect(wrapper.find('[data-track-id="track-100"]').exists()).toBe(false);
  });

  it("uses the window viewport when the list has no measurable height", async () => {
    const tracks = Array.from({ length: 200 }, (_, index) => ({
      ...importedTracks[index % importedTracks.length]!,
      id: `track-${index}`,
      title: `Track ${index}`,
    }));
    const originalInnerHeight = window.innerHeight;
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 1_200,
    });

    try {
      const wrapper = mount(LibraryWindow, {
        props: {
          isUpdating: false,
          snapshot: { ...snapshot, queue: tracks },
        },
      });
      const trackList = wrapper.get("[data-library-track-list]");
      Object.defineProperty(trackList.element, "clientHeight", {
        configurable: true,
        value: 0,
      });
      window.dispatchEvent(new Event("resize"));
      await wrapper.vm.$nextTick();

      expect(
        wrapper.findAll("[data-library-track-virtualizer] [data-track-id]")
          .length,
      ).toBeGreaterThan(26);
    } finally {
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: originalInnerHeight,
      });
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

  it("shows a spinning metadata refresh icon for tracks with incomplete metadata", () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: {
          ...snapshot,
          queue: [
            {
              ...importedTracks[0],
              metadataDirty: true,
            },
          ],
        },
      },
    });

    const indicator = wrapper.get(
      '[data-track-id="M7lc1UVf-VE"] [data-metadata-dirty]',
    );

    expect(indicator.attributes("aria-label")).toBe("Metadata refresh pending");
    expect(indicator.classes()).toContain("animate-spin");
    expect(indicator.text()).toBe("");
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

  it("commits pointer drags from the progress and volume scrubbers", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });
    const volume = wrapper.get('[data-slot="slider"][aria-label="Volume"]');
    const progress = wrapper.get(
      '[data-slot="slider"][aria-label="Track progress"]',
    );

    await dragSlider(volume, 25);
    await dragSlider(progress, 50);

    expect(wrapper.emitted("setVolume")).toEqual([[25]]);
    expect(wrapper.emitted("seek")).toEqual([[119_000]]);
  });

  it("keeps the complete playback footer inline and toggles the queue sidebar", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });
    const footer = wrapper.get("[data-library-playback-footer]");

    expect(
      [...footer.element.children].map((element) =>
        element.getAttribute("data-playback-control"),
      ),
    ).toEqual([
      "now-playing",
      "transport",
      "favorite",
      "progress",
      "volume",
      "settings",
      "queue",
    ]);

    const queueToggle = footer.get('button[aria-label="Show queue"]');
    await queueToggle.trigger("click");

    expect(wrapper.get("[data-library-queue-sidebar]").text()).toContain(
      "Up next",
    );
    expect(
      footer.get('button[aria-label="Hide queue"]').attributes("aria-pressed"),
    ).toBe("true");

    await footer.get('button[aria-label="Hide queue"]').trigger("click");
    expect(wrapper.find("[data-library-queue-sidebar]").exists()).toBe(false);
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
