import { mount } from "@vue/test-utils";
import { ReorderGroup, ReorderItem } from "motion-v";
import { describe, expect, it } from "vitest";

import type { MediaItem } from "@/api";

import QueueWindow from "../QueueWindow.vue";

interface MotionReorderHarness {
  vm: {
    $emit: (event: string, ...args: unknown[]) => void;
    $props: {
      onDragEnd?: () => void;
      onDragStart?: () => void;
    };
  };
}

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

describe("QueueWindow", () => {
  it("renders a virtual Motion queue and routes transport and playback", async () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: "track-1",
        isStarting: false,
        isUpdating: false,
        positionMs: 34_000,
        queue,
        status: "playing",
      },
    });

    expect(wrapper.get("main").attributes("aria-label")).toBe("Play queue");
    expect(wrapper.find("[data-tauri-drag-region]").exists()).toBe(true);
    expect(wrapper.get('[data-current="true"]').text()).toContain("Aerials");
    expect(wrapper.get("[data-queue-summary]").text()).toContain("2 tracks");
    expect(wrapper.get("[data-queue-summary]").text()).toContain("7:33");
    expect(wrapper.find("[data-queue-table]").exists()).toBe(false);
    expect(wrapper.find("thead").exists()).toBe(false);
    expect(
      wrapper
        .find("[data-queue-scroll] [data-slot='scroll-area-viewport']")
        .exists(),
    ).toBe(true);
    expect(wrapper.get("[data-queue-virtualizer]").attributes("role")).toBe(
      "list",
    );
    expect(wrapper.findAll("[data-queue-item]")).toHaveLength(2);
    expect(wrapper.get("[data-queue-footer]").text()).toContain("Aerials");

    await wrapper.get('button[aria-label="Pause playback"]').trigger("click");
    await wrapper.get('button[aria-label="Play Toxicity"]').trigger("click");
    const toxicity = wrapper.get('[data-queue-item][data-queue-index="1"]');
    expect(toxicity.classes()).toContain("cursor-grab");
    expect(toxicity.find('[aria-label="Drag to reorder"]').exists()).toBe(
      false,
    );
    expect(wrapper.findComponent(ReorderGroup).exists()).toBe(true);
    expect(wrapper.findAllComponents(ReorderItem)).toHaveLength(2);

    expect(wrapper.emitted("toggle")).toEqual([[]]);
    expect(wrapper.emitted("playTrack")).toEqual([["track-2"]]);
  });

  it("shows an empty queue without inactive queue actions", () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: undefined,
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        queue: [],
        status: "paused",
      },
    });

    expect(wrapper.text()).toContain("Your queue is empty");
    expect(wrapper.find("[data-queue-list]").exists()).toBe(false);
    expect(
      wrapper.find('[aria-label="Queue playback controls"]').exists(),
    ).toBe(false);
  });

  it("shows the current item first and hides completed items", () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: "track-2",
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        queue: [
          queue[0]!,
          queue[1]!,
          {
            id: "track-3",
            title: "Sugar",
            artist: "System of a Down",
            durationMs: 155_000,
          },
        ],
        status: "playing",
      },
    });

    expect(wrapper.get("[data-queue-summary]").text()).toContain("2 tracks");
    expect(wrapper.find("[data-queue-item-id='track-1']").exists()).toBe(false);
    expect(
      wrapper.get('[data-queue-item][data-queue-index="0"]').text(),
    ).toContain("Toxicity");
    expect(wrapper.get('[data-current="true"]').text()).toContain("Toxicity");
  });

  it("mounts only visible queue rows for a large queue", () => {
    const largeQueue = Array.from({ length: 100 }, (_, index) => ({
      ...queue[index % queue.length]!,
      id: `track-${index}`,
    }));
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: undefined,
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        queue: largeQueue,
        status: "paused",
      },
    });

    expect(wrapper.findAll("[data-queue-item]").length).toBeLessThan(
      largeQueue.length,
    );
    expect(
      wrapper.get("[data-queue-virtualizer]").attributes("style"),
    ).toContain("height: 5200px");
  });

  it("commits Motion reorder output through the authoritative queue command", async () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: undefined,
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        queue,
        status: "paused",
      },
    });

    const reorderGroup = wrapper.getComponent(
      ReorderGroup,
    ) as unknown as MotionReorderHarness;
    const reorderItems = wrapper.findAllComponents(
      ReorderItem,
    ) as unknown as MotionReorderHarness[];
    const toxicity = reorderItems[1]!;

    await toxicity.vm.$props.onDragStart?.();
    await reorderGroup.vm.$emit("update:values", [queue[1]!, queue[0]!]);
    await toxicity.vm.$props.onDragEnd?.();

    expect(wrapper.emitted("move")).toEqual([[1, 0]]);

    await wrapper.setProps({ queue: [queue[1]!, queue[0]!] });
    expect(
      wrapper.get('[data-queue-item][data-queue-index="0"]').text(),
    ).toContain("Toxicity");
  });
});
