import { mount } from "@vue/test-utils";
import { ReorderGroup, ReorderItem } from "motion-v";
import { describe, expect, it } from "vitest";

import type { Playlist } from "@/api";
import LibrarySidebar from "../LibrarySidebar.vue";

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

  it("moves a user playlist title with the accessible controls", async () => {
    const wrapper = mount(LibrarySidebar, {
      props: {
        activeCollection: "tracks",
        isCreatingPlaylist: false,
        isUpdating: false,
        playlists,
      },
    });

    await wrapper
      .get('button[aria-label="Move Road Trip up"]')
      .trigger("click");

    expect(wrapper.emitted("reorderPlaylists")).toEqual([
      [["road-trip", "focus"]],
    ]);
    expect(
      wrapper
        .findAll("[data-playlist-reorder-item]")
        .map((item) => item.attributes("data-playlist-reorder-item")),
    ).toEqual(["road-trip", "focus"]);
  });
});
