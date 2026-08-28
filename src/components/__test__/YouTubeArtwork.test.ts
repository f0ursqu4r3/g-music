import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import YouTubeArtwork from "../YouTubeArtwork.vue";

describe("YouTubeArtwork", () => {
  it("loads max-resolution artwork and falls back to the high-quality image", async () => {
    const wrapper = mount(YouTubeArtwork, {
      props: { videoId: "M7lc1UVf-VE" },
    });

    expect(wrapper.get("img").attributes("src")).toBe(
      "https://i.ytimg.com/vi/M7lc1UVf-VE/maxresdefault.jpg",
    );

    await wrapper.get("img").trigger("error");

    expect(wrapper.get("img").attributes("src")).toBe(
      "https://i.ytimg.com/vi/M7lc1UVf-VE/hqdefault.jpg",
    );
  });

  it("leaves the local artwork fallback visible when both CDN images fail", async () => {
    const wrapper = mount(YouTubeArtwork, {
      props: { videoId: "M7lc1UVf-VE" },
    });

    await wrapper.get("img").trigger("error");
    await wrapper.get("img").trigger("error");

    expect(wrapper.find("img").exists()).toBe(false);
  });

  it("does not request YouTube artwork for non-video identifiers", () => {
    const wrapper = mount(YouTubeArtwork, {
      props: { videoId: "track-1" },
    });

    expect(wrapper.find("img").exists()).toBe(false);
  });
});
