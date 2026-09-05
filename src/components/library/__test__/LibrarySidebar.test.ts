import { mount } from "@vue/test-utils";
import { ReorderGroup, ReorderItem } from "motion-v";
import { describe, expect, it } from "vitest";

import type { Playlist } from "@/api";
import LibrarySidebar from "../LibrarySidebar.vue";
import { LIBRARY_TRACK_IDS_MIME_TYPE } from "../types";

interface MotionReorderHarness {
  vm: {
    $emit: (event: string, ...args: unknown[]) => void;
    $props: {
      onDragEnd?: () => void;
      onDragStart?: () => void;
    };
  };
}

const playlists: Playlist[] = [
  { id: "favorites", name: "Favorites", trackIds: [] },
  { id: "most-played", name: "Most Played", trackIds: [] },
  { id: "focus", name: "Focus", trackIds: [] },
  { id: "road-trip", name: "Road Trip", trackIds: [] },
];

describe("LibrarySidebar", () => {
  it("shows a fixed playlist add action in the Playlist heading", async () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        playlists,
      },
    });

    const addPlaylist = wrapper.get("[data-new-playlist]");
    expect(addPlaylist.attributes("aria-label")).toBe("New playlist");
    expect(addPlaylist.classes()).toContain("shrink-0");

    await addPlaylist.trigger("click");

    expect(wrapper.emitted("newPlaylist")).toEqual([[]]);
  });

  it("clips long playlist names within the sidebar", () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        playlists: [
          ...playlists,
          {
            id: "ambient",
            name: "Cryo Chamber: Dark Ambient Drone Soundscapes",
            trackIds: [],
          },
        ],
      },
    });

    const playlist = wrapper.get('[data-playlist-reorder-item="ambient"]');
    const title = playlist.get("span");

    expect(playlist.classes()).toEqual(
      expect.arrayContaining(["min-w-0", "overflow-hidden"]),
    );
    expect(title.classes()).toEqual(
      expect.arrayContaining(["min-w-0", "flex-1", "truncate"]),
    );
  });

  it("resizes the sidebar with its accessible divider", async () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        playlists,
        sidebarWidth: 244,
      },
    });

    const divider = wrapper.get("[data-library-sidebar-resize]");
    expect(divider.attributes("aria-label")).toBe("Resize library sidebar");
    expect(divider.attributes("aria-valuenow")).toBe("244");

    await divider.trigger("keydown", { key: "ArrowRight" });
    expect(wrapper.emitted("resizeSidebar")).toEqual([[260]]);

    await divider.trigger("mousedown", { button: 0, clientX: 300 });
    window.dispatchEvent(new MouseEvent("mousemove", { clientX: 356 }));
    window.dispatchEvent(new MouseEvent("mouseup"));

    expect(wrapper.emitted("resizeSidebar")).toEqual([[260], [300]]);
  });

  it("colors the selected playlist icon with the accent color", async () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        activePlaylistId: "focus",
        playlists,
      },
    });

    expect(wrapper.get('[data-playlist-id="focus"]').classes()).toContain(
      "aria-[current=page]:[&>svg]:text-accent",
    );

    await wrapper.setProps({ activePlaylistId: "favorites" });

    expect(wrapper.get('[data-playlist-id="favorites"]').classes()).toContain(
      "aria-[current=page]:[&>svg]:text-accent",
    );
  });

  it("uses square full-bleed highlights for every sidebar destination", () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        activePlaylistId: "focus",
        isCreatingPlaylist: true,
        playlists,
      },
    });

    const collection = wrapper.get('[data-collection="tracks"]');
    expect(collection.classes()).toEqual(
      expect.arrayContaining(["-mx-4", "w-[calc(100%+2rem)]", "rounded-none"]),
    );
    expect(collection.classes()).not.toContain("rounded-md");

    const defaultPlaylist = wrapper.get(
      '[data-default-playlist-row="favorites"]',
    );
    expect(defaultPlaylist.classes()).toEqual(
      expect.arrayContaining([
        "-mx-4",
        "w-[calc(100%+2rem)]",
        "rounded-none",
        "data-[current=true]:bg-[oklch(0.7_0.03_262/0.15)]",
      ]),
    );
    expect(defaultPlaylist.classes()).not.toContain("rounded-md");

    const customPlaylist = wrapper.get('[data-playlist-reorder-item="focus"]');
    expect(customPlaylist.attributes("data-current")).toBe("true");
    expect(customPlaylist.classes()).toEqual(
      expect.arrayContaining([
        "-mx-4",
        "w-[calc(100%+2rem)]",
        "rounded-none",
        "data-[current=true]:bg-[oklch(0.7_0.03_262/0.15)]",
      ]),
    );
    expect(customPlaylist.classes()).not.toContain("rounded-md");

    expect(wrapper.get("[data-new-playlist-editor]").classes()).toEqual(
      expect.arrayContaining(["-mx-4", "w-[calc(100%+2rem)]", "rounded-none"]),
    );
  });

  it("clearly highlights an eligible playlist drop target", async () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        playlists,
      },
    });
    const target = wrapper.get('[data-playlist-reorder-item="focus"]');
    const dataTransfer = {
      types: [LIBRARY_TRACK_IDS_MIME_TYPE],
    } as unknown as DataTransfer;

    await target.trigger("dragenter", { dataTransfer });

    expect(target.attributes("data-drop-target")).toBe("true");
    expect(target.classes()).toEqual(
      expect.arrayContaining([
        "data-[drop-target=true]:bg-accent/25",
        "data-[drop-target=true]:ring-1",
        "data-[drop-target=true]:ring-accent",
      ]),
    );
  });

  it("uses full-bleed square highlights in context menus", async () => {
    const wrapper = mount(LibrarySidebar, {
      attachTo: document.body,
      props: {
        activeCollection: "tracks",
        activePlaylistId: "focus",
        playlists,
      },
    });

    await wrapper.get('[data-playlist-id="focus"]').trigger("contextmenu");

    const menu = document.body.querySelector<HTMLElement>(
      "[data-playlist-context-menu]",
    );
    const item = menu?.querySelector<HTMLElement>(
      '[data-slot="context-menu-item"]',
    );
    expect(menu?.classList).toContain("py-1.5");
    expect(menu?.classList).not.toContain("p-1.5");
    expect(item?.classList).toContain("rounded-none");
    expect(menu?.classList).toContain("bg-(--menu-surface)");
    expect(item?.classList).toContain("data-highlighted:bg-(--text)/10");
    expect(item?.classList).not.toContain("rounded-md");

    wrapper.unmount();
  });

  it("commits a dragged user playlist title order", async () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        isCreatingPlaylist: false,
        isUpdating: false,
        playlists,
      },
    });

    const reorderGroup = wrapper.getComponent(
      ReorderGroup,
    ) as unknown as MotionReorderHarness;
    const reorderItems = wrapper.findAllComponents(
      ReorderItem,
    ) as unknown as MotionReorderHarness[];
    const roadTrip = reorderItems[1]!;

    await roadTrip.vm.$props.onDragStart?.();
    await reorderGroup.vm.$emit("update:values", [
      playlists[3]!,
      playlists[2]!,
    ]);

    expect(
      wrapper
        .findAll("[data-playlist-reorder-item]")
        .map((item) => item.attributes("data-playlist-reorder-item")),
    ).toEqual(["road-trip", "focus"]);

    await roadTrip.vm.$props.onDragEnd?.();

    expect(wrapper.emitted("reorderPlaylists")).toEqual([
      [["road-trip", "focus"]],
    ]);

    await wrapper.setProps({
      playlists: [playlists[0]!, playlists[1]!, playlists[3]!, playlists[2]!],
    });

    expect(
      wrapper
        .findAll("[data-playlist-reorder-item]")
        .map((item) => item.attributes("data-playlist-reorder-item")),
    ).toEqual(["road-trip", "focus"]);
  });

  it("uses drag reorder without arrow controls", () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        isCreatingPlaylist: false,
        isUpdating: false,
        playlists,
      },
    });

    expect(wrapper.find('[aria-label="Move Focus up"]').exists()).toBe(false);
    expect(wrapper.find('[aria-label="Move Road Trip down"]').exists()).toBe(
      false,
    );
  });
});
