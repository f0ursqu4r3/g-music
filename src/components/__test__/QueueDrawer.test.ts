import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { MediaItem } from "@/api";

import QueueDrawer from "../QueueDrawer.vue";

const queue: MediaItem[] = [
  {
    id: "track-1",
    title: "Aerials",
    artist: "System of a Down",
    durationMs: 235_000,
  },
  {
    id: "track-2",
    title: "Toxicity",
    artist: "System of a Down",
    durationMs: 218_000,
  },
];

describe("QueueDrawer", () => {
  it("marks the selected track, protects it from queue edits, and emits upcoming moves", async () => {
    const wrapper = mount(QueueDrawer, {
      props: { queue, currentItemId: "track-1", isUpdating: false },
    });

    expect(wrapper.text()).toContain("2 items");
    expect(wrapper.get('[data-current="true"]').text()).toContain("Aerials");
    expect(
      wrapper
        .get('button[aria-label="Move Aerials down"]')
        .attributes("disabled"),
    ).toBeDefined();

    await wrapper.get('button[aria-label="Move Toxicity up"]').trigger("click");

    expect(wrapper.emitted("move")).toEqual([[1, 0]]);
  });

  it("removes only upcoming queue items", async () => {
    const wrapper = mount(QueueDrawer, {
      props: { queue, currentItemId: "track-1", isUpdating: false },
    });

    expect(
      wrapper.find('button[aria-label="Remove Aerials from queue"]').exists(),
    ).toBe(false);
    await wrapper
      .get('button[aria-label="Remove Toxicity from queue"]')
      .trigger("click");

    expect(wrapper.emitted("remove")).toEqual([[1]]);
  });
});
