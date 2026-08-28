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
  it("marks the selected track and emits its move target", async () => {
    const wrapper = mount(QueueDrawer, {
      props: { queue, currentItemId: "track-1", isUpdating: false },
    });

    expect(wrapper.text()).toContain("2 items");
    expect(wrapper.get('[data-current="true"]').text()).toContain("Aerials");

    await wrapper.get('button[aria-label="Move Toxicity up"]').trigger("click");

    expect(wrapper.emitted("move")).toEqual([[1, 0]]);
  });
});
