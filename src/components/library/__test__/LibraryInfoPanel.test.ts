import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { Disc3 } from "lucide-vue-next";

import type { MediaItem } from "@/api";

import LibraryInfoPanel from "../LibraryInfoPanel.vue";
import YouTubeArtwork from "../../YouTubeArtwork.vue";

const selectedTrack: MediaItem = {
  id: "M7lc1UVf-VE",
  title: "Buried Frequencies",
  artist: "Spectral Anomaly",
  durationMs: 120_000,
};

describe("LibraryInfoPanel", () => {
  it("extends track artwork to the top and side edges", () => {
    const wrapper = mount(LibraryInfoPanel, {
      props: {
        isOpen: true,
        selectedTrack,
        selectedAlbum: null,
        selectedArtist: null,
      },
    });

    const artwork = wrapper.get("[data-library-info-artwork]");

    expect(artwork.classes()).toEqual(
      expect.arrayContaining(["-mx-5", "-mt-5", "w-[calc(100%+2.5rem)]"]),
    );
    expect(artwork.classes()).not.toContain("rounded-b-xl");
  });

  it("uses a track icon when selected artwork is unavailable", () => {
    const wrapper = mount(LibraryInfoPanel, {
      props: {
        isOpen: true,
        selectedTrack,
        selectedAlbum: null,
        selectedArtist: null,
      },
    });

    expect(wrapper.getComponent(YouTubeArtwork).props("missingIcon")).toBe(
      Disc3,
    );
  });
});
