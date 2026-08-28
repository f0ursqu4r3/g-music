import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import ImportWindow from "../ImportWindow.vue";

describe("ImportWindow", () => {
  it("owns the Import Music window landmark", () => {
    const wrapper = mount(ImportWindow, {
      props: { isUpdating: false },
    });

    expect(wrapper.get("main").attributes("aria-label")).toBe("Import music");
    expect(wrapper.get("h1").text()).toBe("Import Music");
  });

  it("submits many unique video, playlist, and artist URLs", async () => {
    const wrapper = mount(ImportWindow, {
      props: { isUpdating: false },
    });
    const input = wrapper.get('textarea[aria-label="YouTube URLs"]');

    await input.setValue(
      [
        "https://youtu.be/M7lc1UVf-VE",
        "https://youtube.com/playlist?list=PL-example",
        "https://youtube.com/@artist/videos",
        "https://youtu.be/M7lc1UVf-VE",
      ].join("\n"),
    );
    await wrapper
      .get('form[aria-label="Import music from YouTube"]')
      .trigger("submit");

    expect(wrapper.emitted("importYoutubeUrls")).toEqual([
      [
        [
          "https://youtu.be/M7lc1UVf-VE",
          "https://youtube.com/playlist?list=PL-example",
          "https://youtube.com/@artist/videos",
        ],
      ],
    ]);
    expect(input.attributes("placeholder")).toContain("one URL per line");
    expect((input.element as HTMLTextAreaElement).value).toBe("");
  });

  it("shows import errors in the Import Music window", () => {
    const wrapper = mount(ImportWindow, {
      props: {
        errorMessage: "could not resolve YouTube metadata",
        isUpdating: false,
      },
    });

    expect(wrapper.get('[role="alert"]').text()).toBe(
      "could not resolve YouTube metadata",
    );
  });
});
