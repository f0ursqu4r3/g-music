import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";

import type { MediaItem, PlaybackSnapshot, PlaybackTransport } from "@/api";
import { Slider } from "@/components/ui/slider";
import LibrarySidebar from "../library/LibrarySidebar.vue";
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
    const albumTable = wrapper.get("[data-library-album-list]");
    expect(albumTable.text()).toContain("Album");
    expect(albumTable.text()).toContain("Artist");
    expect(albumTable.text()).toContain("Tracks");
    expect(albumTable.text()).toContain("Duration");
    expect(albumTable.findAll("tbody .album-tile")).toHaveLength(2);
    expect(wrapper.text()).toContain("API Sessions");

    await wrapper.get('[data-collection="artists"]').trigger("click");
    expect(wrapper.get("h1").text()).toBe("Artists");
    const artistTable = wrapper.get("[data-library-artist-list]");
    expect(artistTable.text()).toContain("Artist");
    expect(artistTable.text()).toContain("Albums");
    expect(artistTable.text()).toContain("Tracks");
    expect(artistTable.text()).toContain("Duration");
    expect(artistTable.findAll("tbody .artist-tile")).toHaveLength(2);
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

  it("sorts tracks from table column headers", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const titleHeader = wrapper.get('[data-sort-column="title"]');
    expect(titleHeader.attributes("aria-sort")).toBe("ascending");

    await titleHeader.get("button").trigger("click");

    expect(titleHeader.attributes("aria-sort")).toBe("descending");
    expect(
      wrapper.get('[data-track-id="M7lc1UVf-VE"] .track-title').text(),
    ).toBe("YouTube Developers Live");

    for (const column of ["artist", "album", "duration"] as const) {
      const header = wrapper.get(`[data-sort-column="${column}"]`);

      await header.get("button").trigger("click");

      expect(header.attributes("aria-sort")).toBe("ascending");
    }
  });

  it("sorts album and artist tables from every data column header", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-collection="albums"]').trigger("click");
    for (const [column, direction] of [
      ["title", "descending"],
      ["artist", "ascending"],
      ["track-count", "ascending"],
      ["duration", "ascending"],
    ] as const) {
      const header = wrapper
        .get("[data-library-album-list]")
        .get(`[data-sort-column="${column}"]`);

      await header.get("button").trigger("click");

      expect(header.attributes("aria-sort")).toBe(direction);
    }

    await wrapper.get('[data-collection="artists"]').trigger("click");
    for (const column of [
      "title",
      "album-count",
      "track-count",
      "duration",
    ] as const) {
      const header = wrapper
        .get("[data-library-artist-list]")
        .get(`[data-sort-column="${column}"]`);

      await header.get("button").trigger("click");

      expect(header.attributes("aria-sort")).toBe("ascending");
    }
  });

  it("orders album and artist rows by their count columns", async () => {
    const countTracks: MediaItem[] = [
      { ...importedTracks[0], album: "Big", artist: "Band A", id: "big-1" },
      { ...importedTracks[0], album: "Big", artist: "Band A", id: "big-2" },
      { ...importedTracks[1], album: "Small", artist: "Band B", id: "small-1" },
      {
        ...importedTracks[0],
        album: "Medium",
        artist: "Band A",
        id: "medium-1",
      },
    ];
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: {
          ...snapshot,
          currentItem: countTracks[0],
          queue: countTracks,
        },
      },
    });

    await wrapper.get('[data-collection="albums"]').trigger("click");
    const albumTable = wrapper.get("[data-library-album-list]");
    await albumTable
      .get('[data-sort-column="track-count"] button')
      .trigger("click");
    expect(albumTable.findAll("tbody .album-tile")[0]!.text()).toContain(
      "Small",
    );

    await albumTable
      .get('[data-sort-column="track-count"] button')
      .trigger("click");
    expect(albumTable.findAll("tbody .album-tile")[0]!.text()).toContain("Big");

    await wrapper.get('[data-collection="artists"]').trigger("click");
    const artistTable = wrapper.get("[data-library-artist-list]");
    await artistTable
      .get('[data-sort-column="album-count"] button')
      .trigger("click");
    expect(artistTable.findAll("tbody .artist-tile")[0]!.text()).toContain(
      "Band B",
    );

    await artistTable
      .get('[data-sort-column="album-count"] button')
      .trigger("click");
    expect(artistTable.findAll("tbody .artist-tile")[0]!.text()).toContain(
      "Band A",
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

  it("edits the selected track metadata in a modal", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="M7lc1UVf-VE"]').trigger("click");
    await wrapper
      .get('[aria-label="Edit track YouTube Developers Live"]')
      .trigger("click");
    expect(wrapper.get('[role="dialog"]').text()).toContain("Edit Track");
    expect(wrapper.get('[role="dialog"]').classes()).toContain("bg-black/60");
    expect(wrapper.get('[role="dialog"] form').classes()).toContain(
      "bg-[oklch(0.11_0.014_260/0.98)]",
    );

    await wrapper
      .get('[data-metadata-field="title"]')
      .setValue("Renamed session");
    await wrapper.get('[role="dialog"] form').trigger("submit");

    expect(wrapper.emitted("updateTracksMetadata")).toEqual([
      [
        [
          {
            id: "M7lc1UVf-VE",
            metadata: {
              album: "API Sessions",
              artist: "Google for Developers",
              genres: [],
              label: null,
              title: "Renamed session",
            },
          },
        ],
      ],
    ]);
  });

  it("edits album metadata across its tracks without changing their titles", async () => {
    const albumTracks: MediaItem[] = [
      { ...importedTracks[0], album: "Shared", id: "shared-1", title: "First" },
      {
        ...importedTracks[0],
        album: "Shared",
        id: "shared-2",
        title: "Second",
      },
    ];
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: {
          ...snapshot,
          currentItem: albumTracks[0],
          queue: albumTracks,
        },
      },
    });

    await wrapper.get('[data-collection="albums"]').trigger("click");
    await wrapper.get(".album-tile").trigger("click");
    await wrapper.get('[aria-label="Edit album Shared"]').trigger("click");
    await wrapper
      .get('[data-metadata-field="album"]')
      .setValue("Renamed album");
    await wrapper.get('[role="dialog"] form').trigger("submit");

    expect(wrapper.emitted("updateTracksMetadata")).toEqual([
      [
        [
          {
            id: "shared-1",
            metadata: {
              album: "Renamed album",
              artist: "Google for Developers",
              genres: [],
              label: null,
              title: "First",
            },
          },
          {
            id: "shared-2",
            metadata: {
              album: "Renamed album",
              artist: "Google for Developers",
              genres: [],
              label: null,
              title: "Second",
            },
          },
        ],
      ],
    ]);
  });

  it("edits artist metadata across that artist's tracks", async () => {
    const artistTracks: MediaItem[] = [
      { ...importedTracks[0], id: "artist-1", title: "First" },
      { ...importedTracks[0], id: "artist-2", title: "Second" },
    ];
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: {
          ...snapshot,
          currentItem: artistTracks[0],
          queue: artistTracks,
        },
      },
    });

    await wrapper.get('[data-collection="artists"]').trigger("click");
    await wrapper.get(".artist-tile").trigger("click");
    await wrapper
      .get('[aria-label="Edit artist Google for Developers"]')
      .trigger("click");
    await wrapper
      .get('[data-metadata-field="artist"]')
      .setValue("Renamed artist");
    await wrapper.get('[role="dialog"] form').trigger("submit");

    expect(wrapper.emitted("updateTracksMetadata")).toEqual([
      [
        [
          {
            id: "artist-1",
            metadata: {
              album: "API Sessions",
              artist: "Renamed artist",
              genres: [],
              label: null,
              title: "First",
            },
          },
          {
            id: "artist-2",
            metadata: {
              album: "API Sessions",
              artist: "Renamed artist",
              genres: [],
              label: null,
              title: "Second",
            },
          },
        ],
      ],
    ]);
  });

  it("shows complete metadata and listening history for a selected track", async () => {
    const detailedTrack: MediaItem = {
      ...importedTracks[0],
      albumArtist: "Google",
      availability: "public",
      categories: ["Science & Technology", "Music"],
      channel: "Google Developers",
      channelId: "UC_x5XG1OV2P6uZZ5FSM9Ttw",
      description: "A complete metadata fixture.",
      discNumber: 1,
      genres: ["Educational"],
      isLive: false,
      language: "en",
      lastPlayedAtMs: 1_734_000_060_000,
      likeCount: 7,
      metadataDirty: false,
      playCount: 2,
      playHistoryMs: [1_734_000_000_000, 1_734_000_060_000],
      provider: "youtube",
      releaseDate: "2025-01-02",
      sourceUrl: "https://www.youtube.com/watch?v=M7lc1UVf-VE",
      tags: ["API", "Developers"],
      thumbnailUrl: "https://i.ytimg.com/vi/M7lc1UVf-VE/maxresdefault.jpg",
      trackNumber: 3,
      uploadDate: "2025-01-03",
      uploader: "Google for Developers",
      uploaderId: "GoogleDevelopers",
      viewCount: 42,
    };
    const detailedSnapshot = {
      ...snapshot,
      currentItem: detailedTrack,
      queue: [detailedTrack, importedTracks[1]],
    };
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot: detailedSnapshot },
    });

    await wrapper.get('[data-track-id="M7lc1UVf-VE"]').trigger("click");

    const sidebar = wrapper.get('[data-library-info="track"]');
    for (const text of [
      "Track number",
      "Disc number",
      "Uploaded",
      "Description",
      "Channel ID",
      "Uploader",
      "Uploader ID",
      "Categories",
      "Tags",
      "Language",
      "Availability",
      "Views",
      "Likes",
      "Provider",
      "Play count",
      "Last played",
      "Play history",
    ]) {
      expect(sidebar.text()).toContain(text);
    }
    expect(sidebar.get("[data-track-source]").attributes("href")).toBe(
      detailedTrack.sourceUrl,
    );
    expect(sidebar.get("[data-track-thumbnail]").attributes("href")).toBe(
      detailedTrack.thumbnailUrl,
    );
    expect(sidebar.findAll("[data-play-history] li")).toHaveLength(2);
  });

  it("expands and collapses a long selected-track description", async () => {
    const detailedTrack: MediaItem = {
      ...importedTracks[0],
      description: "Long description. ".repeat(30),
    };
    const detailedSnapshot = {
      ...snapshot,
      currentItem: detailedTrack,
      queue: [detailedTrack, importedTracks[1]],
    };
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot: detailedSnapshot },
    });

    await wrapper.get('[data-track-id="M7lc1UVf-VE"]').trigger("click");

    const description = wrapper.get("[data-track-description]");
    const toggle = wrapper.get("[data-track-description-toggle]");
    expect(description.classes()).toContain("line-clamp-6");
    expect(toggle.text()).toBe("Show more");
    expect(toggle.attributes("aria-expanded")).toBe("false");

    await toggle.trigger("click");
    expect(description.classes()).not.toContain("line-clamp-6");
    expect(toggle.text()).toBe("Show less");
    expect(toggle.attributes("aria-expanded")).toBe("true");

    await toggle.trigger("click");
    expect(description.classes()).toContain("line-clamp-6");
    expect(toggle.text()).toBe("Show more");
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

  it("uses shared scroll areas for library content surfaces", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const sidebar = wrapper.get("[data-library-selected-sidebar]");
    expect(sidebar.find("[data-slot='scroll-area-viewport']").exists()).toBe(
      true,
    );
    expect(
      wrapper
        .get("[data-library-sidebar]")
        .find("[data-slot='scroll-area-viewport']")
        .exists(),
    ).toBe(true);
    expect(
      wrapper
        .get("[data-library-track-list]")
        .find("[data-slot='scroll-area-viewport']")
        .exists(),
    ).toBe(true);
    await wrapper.get('button[aria-label="Grid view"]').trigger("click");
    expect(
      wrapper
        .get('[aria-label="Tracks grid"]')
        .find("[data-slot='scroll-area-viewport']")
        .exists(),
    ).toBe(true);
  });

  it("keeps playback footer controls within the smallest library window", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });
    const footer = wrapper.get("[data-library-playback-footer]");

    expect(footer.classes()).toContain("min-w-0");
    expect(footer.classes()).toContain("max-[920px]:grid");
    expect(footer.classes()).not.toContain("min-w-max");
    expect(footer.element.closest("[data-slot='scroll-area']")).toBeNull();
  });

  it("uses the compact reference-style library header and track table", () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: { ...snapshot, status: "playing" },
      },
    });

    expect(wrapper.text()).not.toContain("Your library");
    expect(wrapper.text()).not.toContain(
      "Artwork and queue are in the Window menu",
    );
    expect(wrapper.get('[aria-label="Library view options"]')).toBeDefined();
    expect(
      wrapper.findAll('[aria-label="Library view options"] button'),
    ).toHaveLength(3);
    expect(wrapper.findAll("thead th")).toHaveLength(6);
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
    const trackList = wrapper.get(
      "[data-library-track-list] [data-slot='scroll-area-viewport']",
    );

    expect(wrapper.get("[data-library-track-header]").classes()).toContain(
      "shrink-0",
    );
    expect(wrapper.get("thead").classes()).not.toContain("sticky");
    expect(wrapper.find("[data-library-track-header-fade]").exists()).toBe(
      false,
    );
    expect(trackList.classes()).toContain("library-track-scroll");
    expect(wrapper.find("[data-library-track-content-fade]").exists()).toBe(
      false,
    );
    expect(wrapper.findAll("tbody [data-track-id]").length).toBeLessThan(
      tracks.length,
    );

    trackList.element.scrollTop = 36 * 100;
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
    const trackList = wrapper.get(
      "[data-library-track-list] [data-slot='scroll-area-viewport']",
    );

    trackList.element.scrollTop = 36 * 100;
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
      const trackList = wrapper.get(
        "[data-library-track-list] [data-slot='scroll-area-viewport']",
      );
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

  it("selects a track for metadata without starting playback", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("click");

    expect(wrapper.get('[data-library-info="track"]').text()).toContain(
      "Creator Studio Session",
    );
    expect(wrapper.emitted("playTrack")).toBeUndefined();
  });

  it("routes shuffle and repeat mode controls through the library command boundary", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('button[aria-label="Enable shuffle"]').trigger("click");
    await wrapper
      .get('button[aria-label="Enable repeat all"]')
      .trigger("click");

    expect(wrapper.emitted("toggleShuffle")).toEqual([[]]);
    expect(wrapper.emitted("cycleRepeatMode")).toEqual([[]]);
  });

  it("shows the active shuffle and repeat modes with distinct icons", async () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        snapshot: { ...snapshot, repeatMode: "all", shuffleEnabled: true },
      },
    });

    const shuffle = wrapper.get('button[aria-label="Disable shuffle"]');
    const repeat = wrapper.get('button[aria-label="Enable repeat one"]');
    expect(shuffle.attributes("aria-pressed")).toBe("true");
    expect(repeat.attributes("data-repeat-mode")).toBe("all");
    expect(repeat.find("svg").classes()).toContain("lucide-repeat-2");

    await wrapper.setProps({
      snapshot: { ...snapshot, repeatMode: "one", shuffleEnabled: true },
      isUpdating: true,
    });

    const repeatOne = wrapper.get('button[aria-label="Disable repeat"]');
    expect(repeatOne.attributes("data-repeat-mode")).toBe("one");
    expect(repeatOne.find("svg").classes()).toContain("lucide-repeat-1");
    expect(shuffle.attributes("disabled")).toBeDefined();
    expect(repeatOne.attributes("disabled")).toBeDefined();
  });

  it("highlights the selected track separately from the playing track", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("click");

    expect(
      wrapper.get('[data-track-id="BaW_jenozKc"]').attributes("data-selected"),
    ).toBe("true");
    expect(
      wrapper.get('[data-track-id="M7lc1UVf-VE"]').attributes("data-selected"),
    ).toBe("false");
    expect(wrapper.find(".track-playing-indicator").exists()).toBe(false);

    await wrapper.setProps({ snapshot: { ...snapshot, status: "playing" } });

    expect(
      wrapper.get('[data-track-id="M7lc1UVf-VE"]').attributes("data-playing"),
    ).toBe("true");
    expect(
      wrapper
        .get('[data-track-id="M7lc1UVf-VE"]')
        .find(".track-playing-indicator")
        .exists(),
    ).toBe(true);

    await wrapper.get('button[aria-label="Grid view"]').trigger("click");

    expect(
      wrapper
        .get('.track-grid [data-track-id="BaW_jenozKc"]')
        .attributes("data-selected"),
    ).toBe("true");
    expect(
      wrapper
        .get('.track-grid [data-track-id="M7lc1UVf-VE"]')
        .attributes("data-playing"),
    ).toBe("true");
    expect(
      wrapper
        .get('.track-grid [data-track-id="M7lc1UVf-VE"]')
        .find(".track-playing-indicator")
        .exists(),
    ).toBe(true);
  });

  it("creates a one-item queue when playing from the unfiltered Tracks list", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const row = wrapper.get('[data-track-id="BaW_jenozKc"]');
    const playButton = row.get(
      'button[aria-label="Play Creator Studio Session"]',
    );
    expect(playButton.attributes("data-track-action")).toBe("play");
    expect(playButton.classes()).toContain("opacity-0");
    expect(playButton.classes()).toContain("group-hover:opacity-100");
    expect(playButton.classes()).not.toContain("rounded-full");
    expect(playButton.classes()).not.toContain(
      "bg-[oklch(0.72_0.04_268/0.92)]",
    );

    await playButton.trigger("click");
    await row.trigger("dblclick");

    expect(wrapper.emitted("playTrack")).toEqual([
      [["BaW_jenozKc"], "BaW_jenozKc"],
      [["BaW_jenozKc"], "BaW_jenozKc"],
    ]);
  });

  it("adds the selected track to the front or end of the play queue", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("click");
    await wrapper
      .get('button[aria-label="Play Creator Studio Session next"]')
      .trigger("click");
    await wrapper
      .get('button[aria-label="Add Creator Studio Session to queue"]')
      .trigger("click");

    expect(wrapper.emitted("playNext")).toEqual([["BaW_jenozKc"]]);
    expect(wrapper.emitted("addToQueue")).toEqual([["BaW_jenozKc"]]);
  });

  it("opens a track action menu with the current library actions", async () => {
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("contextmenu");

    const menu = document.body.querySelector("[data-track-context-menu]");
    expect(menu?.textContent).toContain("Play");
    expect(menu?.textContent).toContain("Play next");
    expect(menu?.textContent).toContain("Add to queue");
    expect(menu?.textContent).toContain("Add to Favorites");
    expect(menu?.textContent).toContain("Go to album");
    expect(menu?.textContent).toContain("Go to artist");
    expect(menu?.textContent).toContain("Edit metadata");
    expect(menu?.textContent).toContain("Remove from library…");

    wrapper.unmount();
  });

  it("routes a track context action through the library command", async () => {
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("contextmenu");
    await document
      .querySelector<HTMLButtonElement>(
        '[data-track-context-action="play-next"]',
      )
      ?.click();

    expect(wrapper.emitted("playNext")).toEqual([["BaW_jenozKc"]]);

    wrapper.unmount();
  });

  it("requires confirmation before removing a track from the durable library", async () => {
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('[data-track-id="BaW_jenozKc"]').trigger("contextmenu");
    await document
      .querySelector<HTMLButtonElement>('[data-track-context-action="remove"]')
      ?.click();

    expect(wrapper.get('[role="dialog"]').text()).toContain(
      "Remove from library?",
    );
    expect(wrapper.emitted("removeTracks")).toBeUndefined();

    await wrapper.get("[data-confirm-track-removal]").trigger("click");
    expect(wrapper.emitted("removeTracks")).toEqual([[["BaW_jenozKc"]]]);
    wrapper.unmount();
  });

  it("opens contextual actions from track grid, collection rows, and playlists", async () => {
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: {
        isUpdating: false,
        playlists: [
          { id: "focus", name: "Focus", trackIds: [importedTracks[0]!.id] },
        ],
        snapshot,
      },
    });

    await wrapper.get('button[aria-label="Grid view"]').trigger("click");
    await wrapper.get(".track-grid .track-tile").trigger("contextmenu");
    expect(document.querySelector("[data-track-context-menu]")).not.toBeNull();

    await wrapper.get('[data-collection="albums"]').trigger("click");
    await wrapper.get(".album-tile").trigger("contextmenu");
    expect(document.querySelector("[data-album-context-menu]")).not.toBeNull();

    await wrapper.get('[data-collection="artists"]').trigger("click");
    await wrapper.get(".artist-tile").trigger("contextmenu");
    expect(document.querySelector("[data-artist-context-menu]")).not.toBeNull();

    await wrapper.get('[data-playlist-id="focus"]').trigger("contextmenu");
    expect(
      document.querySelector("[data-playlist-context-menu]"),
    ).not.toBeNull();

    wrapper.unmount();
  });

  it("uses dense square-corner track rows", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });
    const row = wrapper.get('[data-track-id="BaW_jenozKc"]');

    expect(row.classes()).toContain("h-9");
    expect(
      row.get(".track-title").element.closest('[role="gridcell"]')?.classList,
    ).toContain("px-3");
    for (const cell of row.findAll('[role="gridcell"]')) {
      expect(cell.classes()).not.toContain("rounded-l-md");
      expect(cell.classes()).not.toContain("rounded-r-md");
    }
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
      "Enable shuffle",
      "Previous track",
      "Play",
      "Next track",
      "Enable repeat all",
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
    volume!.vm.$emit("update:modelValue", [72]);
    expect(wrapper.emitted("setVolume")).toEqual([[72]]);
  });

  it("shows a loading spinner in the main play button while playback starts", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isStarting: true, isUpdating: true, snapshot },
    });

    const playButton = wrapper.get('button[aria-label="Starting playback"]');
    expect(playButton.attributes("aria-busy")).toBe("true");
    expect(playButton.get("[data-playback-starting]").classes()).toContain(
      "animate-spin",
    );
  });

  it("mutes and restores the prior library volume from its icon", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    await wrapper.get('button[aria-label="Mute volume"]').trigger("click");
    expect(wrapper.emitted("toggleMute")).toEqual([[]]);

    await wrapper.setProps({ snapshot: { ...snapshot, volumePercent: 0 } });
    await wrapper.get('button[aria-label="Unmute volume"]').trigger("click");

    expect(wrapper.emitted("toggleMute")).toEqual([[], []]);
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

  it("keeps selected-item details in an animated right sidebar", async () => {
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
      "details",
    ]);

    const sidebar = wrapper.get("[data-library-selected-sidebar]");
    expect(sidebar.text()).toContain("YouTube Developers Live");
    expect(sidebar.text()).not.toContain("Up next");
    expect(sidebar.classes()).toContain("transition-all");

    const detailsToggle = footer.get(
      'button[aria-label="Show selection details"]',
    );
    expect(detailsToggle.attributes("aria-pressed")).toBe("false");
    expect(sidebar.classes()).toContain("opacity-0");

    await detailsToggle.trigger("click");
    expect(
      footer
        .get('button[aria-label="Hide selection details"]')
        .attributes("aria-pressed"),
    ).toBe("true");
    expect(sidebar.classes()).toContain("opacity-100");

    await footer
      .get('button[aria-label="Hide selection details"]')
      .trigger("click");
    expect(sidebar.classes()).toContain("opacity-0");
  });

  it("delegates track favorite changes from the table", async () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    const favorite = wrapper.get(
      'button[aria-label="Favorite Creator Studio Session"]',
    );
    expect(favorite.attributes("aria-pressed")).toBe("false");

    await favorite.trigger("click");

    expect(wrapper.emitted("toggleFavorite")).toEqual([["BaW_jenozKc"]]);
  });

  it("keeps the left sidebar navigation controls", () => {
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, snapshot },
    });

    expect(wrapper.findAll("[data-collection]")).toHaveLength(3);
    expect(wrapper.get("[data-library-sidebar]").text()).toContain("Library");
  });

  it("shows default playlists and filters tracks by their stable IDs", async () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        playlists: [
          {
            id: "favorites",
            name: "Favorites",
            trackIds: ["BaW_jenozKc"],
          },
          { id: "most-played", name: "Most Played", trackIds: ["M7lc1UVf-VE"] },
        ],
        snapshot,
      },
    });

    expect(wrapper.get('[data-playlist-id="favorites"]').text()).toContain(
      "Favorites",
    );
    expect(wrapper.get('[data-playlist-id="most-played"]').text()).toContain(
      "Most Played",
    );

    await wrapper.get('[data-playlist-id="favorites"]').trigger("click");

    expect(wrapper.get("h1").text()).toBe("Favorites");
    expect(wrapper.findAll("[data-track-id]")).toHaveLength(1);
    expect(wrapper.get('[data-track-id="BaW_jenozKc"]').text()).toContain(
      "Creator Studio Session",
    );
  });

  it("plays a selected playlist in its visible sorted order", async () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        playlists: [
          {
            id: "focus",
            name: "Focus",
            trackIds: ["M7lc1UVf-VE", "BaW_jenozKc"],
          },
        ],
        snapshot,
      },
    });

    await wrapper.get('[data-playlist-id="focus"]').trigger("click");

    expect(
      wrapper.get('[data-playlist-id="focus"]').attributes("aria-current"),
    ).toBe("page");
    expect(
      wrapper.get('[data-collection="tracks"]').attributes("aria-current"),
    ).toBeUndefined();

    await wrapper.get('button[aria-label="Play Focus"]').trigger("click");

    expect(wrapper.emitted("playTrack")).toEqual([
      [["BaW_jenozKc", "M7lc1UVf-VE"], "BaW_jenozKc"],
    ]);
  });

  it("delegates a reordered user playlist list", async () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        playlists: [
          { id: "favorites", name: "Favorites", trackIds: [] },
          { id: "most-played", name: "Most Played", trackIds: [] },
          { id: "focus", name: "Focus", trackIds: [] },
          { id: "road-trip", name: "Road Trip", trackIds: [] },
        ],
        snapshot,
      },
    });

    wrapper
      .getComponent(LibrarySidebar)
      .vm.$emit("reorderPlaylists", ["road-trip", "focus"]);

    expect(wrapper.emitted("reorderPlaylists")).toEqual([
      [["road-trip", "focus"]],
    ]);
  });

  it("shows the inline new-playlist editor after existing playlists", async () => {
    const wrapper = mount(LibraryWindow, {
      props: {
        isUpdating: false,
        playlists: [
          { id: "focus", name: "Focus", trackIds: [] },
          { id: "road-trip", name: "Road Trip", trackIds: [] },
        ],
        snapshot,
      },
    });

    await wrapper.get('button[aria-label="New playlist"]').trigger("click");
    const editor = wrapper.get("[data-new-playlist-editor]");

    expect(
      wrapper
        .get("[data-library-playlists]")
        .findAll("[data-playlist-id], [data-new-playlist-editor]")
        .map((element) => element.attributes("data-playlist-id") ?? "editor"),
    ).toEqual(["focus", "road-trip", "editor"]);

    await editor
      .get('input[aria-label="New playlist name"]')
      .setValue("Road Trip");
    await editor.trigger("submit");

    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    expect(wrapper.emitted("upsertPlaylist")).toEqual([
      [
        expect.objectContaining({
          name: "Road Trip",
          trackIds: [],
        }),
      ],
    ]);
  });

  it("edits and deletes a user playlist", async () => {
    const playlist = {
      id: "focus",
      name: "Focus",
      trackIds: ["M7lc1UVf-VE"],
    };
    const wrapper = mount(LibraryWindow, {
      props: { isUpdating: false, playlists: [playlist], snapshot },
    });

    await wrapper.get('button[aria-label="Edit Focus"]').trigger("click");
    const dialog = wrapper.get('[role="dialog"]');
    await dialog.get('[data-playlist-field="name"]').setValue("Deep Focus");
    await dialog
      .get('[data-playlist-track="BaW_jenozKc"] input')
      .setValue(true);
    await dialog.get("form").trigger("submit");

    expect(wrapper.emitted("upsertPlaylist")).toEqual([
      [
        {
          id: "focus",
          name: "Deep Focus",
          trackIds: ["M7lc1UVf-VE", "BaW_jenozKc"],
        },
      ],
    ]);

    await wrapper.get('button[aria-label="Edit Focus"]').trigger("click");
    await wrapper.get("[data-playlist-editor-delete]").trigger("click");
    await wrapper.get("[data-playlist-editor-delete]").trigger("click");

    expect(wrapper.emitted("deletePlaylist")).toEqual([["focus"]]);
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
