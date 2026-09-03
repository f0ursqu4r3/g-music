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
