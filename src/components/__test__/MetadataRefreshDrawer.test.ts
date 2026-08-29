import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import MetadataRefreshDrawer from "../MetadataRefreshDrawer.vue";

describe("MetadataRefreshDrawer", () => {
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
  });
});
