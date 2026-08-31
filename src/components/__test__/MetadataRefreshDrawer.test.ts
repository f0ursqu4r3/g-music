import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import MetadataRefreshDrawer from "../MetadataRefreshDrawer.vue";

describe("MetadataRefreshDrawer", () => {
  it("bounds the refresh drawer so its job list can scroll", () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          completedTracks: 0,
          jobs: [],
          totalTracks: 0,
        },
      },
    });

    expect(wrapper.get('[aria-label="Metadata refreshes"]').classes()).toEqual(
      expect.arrayContaining(["top-0", "bottom-0"]),
    );
  });

  it("shows queued and active dirty-track refreshes", () => {
    const wrapper = mount(MetadataRefreshDrawer, {
      props: {
        refreshes: {
          completedTracks: 3,
          jobs: [
            {
              message: "Waiting for metadata refresh.",
              state: "queued",
              trackId: "M7lc1UVf-VE",
              title: "Discovered track",
            },
            {
              message: "Fetching full YouTube metadata.",
              state: "refreshing",
              trackId: "BaW_jenozKc",
              title: "Playing track",
            },
          ],
          totalTracks: 5,
        },
      },
    });

    expect(wrapper.get('[aria-label="Metadata refreshes"]').text()).toContain(
      "3 of 5 refreshed",
    );
    expect(
      wrapper.get('[data-refresh-track-id="M7lc1UVf-VE"]').text(),
    ).toContain("Queued");
    expect(
      wrapper.get('[data-refresh-track-id="BaW_jenozKc"]').text(),
    ).toContain("Refreshing");
    expect(wrapper.find("[data-slot='scroll-area-viewport']").exists()).toBe(
      true,
    );
  });
});
