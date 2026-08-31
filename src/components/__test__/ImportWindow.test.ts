import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import ImportWindow from "../ImportWindow.vue";

describe("ImportWindow", () => {
  it("owns the Import Music window landmark", () => {
    const wrapper = mount(ImportWindow, {
      props: { isImporting: false },
    });

    expect(wrapper.get("main").attributes("aria-label")).toBe("Import music");
    expect(wrapper.get("h1").text()).toBe("Import Music");
  });

  it("submits many unique video, playlist, and artist URLs", async () => {
    const wrapper = mount(ImportWindow, {
      props: { isImporting: false },
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
        isImporting: false,
      },
    });

    expect(wrapper.get('[role="alert"]').text()).toBe(
      "could not resolve YouTube metadata",
    );
  });

  it("shows import progress and terminal output", async () => {
    const wrapper = mount(ImportWindow, {
      props: {
        isImporting: true,
        progress: {
          completedSources: 1,
          importedTracks: 12,
          message: "Resolved source 1 of 2; 12 track(s) found.",
          phase: "resolving",
          runId: 4,
          skippedMemberOnly: 3,
          totalSources: 2,
        },
      },
    });

    await wrapper.vm.$nextTick();

    expect(wrapper.get("progress").attributes("value")).toBe("50");
    expect(wrapper.get('[role="log"]').text()).toContain(
      "Resolved source 1 of 2; 12 track(s) found.",
    );
    expect(wrapper.findAll("[data-slot='scroll-area-viewport']")).toHaveLength(
      2,
    );
    expect(wrapper.text()).toContain("3 members-only track(s) skipped");
    expect(wrapper.get("textarea").attributes("disabled")).toBeDefined();
  });

  it("shows indeterminate progress while the first source streams track metadata", () => {
    const wrapper = mount(ImportWindow, {
      props: {
        isImporting: true,
        progress: {
          completedSources: 0,
          importedTracks: 12,
          message: "Found 12 track(s) while reading source 1 of 1…",
          phase: "resolving",
          runId: 5,
          skippedMemberOnly: 0,
          totalSources: 1,
        },
      },
    });

    expect(wrapper.get("progress").attributes("value")).toBeUndefined();
    expect(wrapper.text()).toContain("12 track(s) found");
  });
});
