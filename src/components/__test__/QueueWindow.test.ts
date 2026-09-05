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
  it("reserves content-sized duration space before row actions for long tracks", () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: undefined,
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        status: "paused",
        queue: [{ ...queue[0]!, durationMs: 7_200_000 }],
      },
    });
    const row = wrapper.get("[data-queue-item]");
    expect(row.classes()).toContain(
      "grid-cols-[2.25rem_minmax(0,1fr)_max-content_2.25rem]",
    );
    const duration = row.get("[data-queue-duration]");
    expect(duration.text()).toBe("2:00:00");
    expect(duration.classes()).toContain("whitespace-nowrap");
    expect(duration.classes()).toContain("pr-2");
    expect(duration.classes()).toContain("text-right");
    wrapper.unmount();
  });

  it("keeps upcoming row actions hidden independently of disabled styling while advancing", async () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: "track-1",
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        status: "playing",
        queue: [...queue, { ...queue[0]!, id: "track-3", title: "Sugar" }],
      },
    });
    await wrapper.get('[aria-label="Next track"]').trigger("click");
    expect(wrapper.emitted("next")).toEqual([[]]);
    for (const state of [
      { currentItemId: "track-1", isStarting: true, isUpdating: false },
      { currentItemId: "track-2", isStarting: true, isUpdating: false },
      { currentItemId: "track-2", isStarting: false, isUpdating: true },
      { currentItemId: "track-2", isStarting: false, isUpdating: false },
    ]) {
      await wrapper.setProps(state);
      const current = wrapper.get('[data-current="true"] button');
      expect(
        current.element.parentElement?.classList.contains("opacity-0"),
      ).toBe(false);
      for (const button of wrapper.findAll('[data-current="false"] button')) {
        const shell = button.element.parentElement!;
        expect(shell.classList.contains("opacity-0")).toBe(true);
        expect(shell.classList.contains("group-hover:opacity-100")).toBe(true);
        expect(shell.classList.contains("group-focus-within:opacity-100")).toBe(
          true,
        );
        expect(shell.hasAttribute("disabled")).toBe(false);
        expect(button.attributes("disabled") !== undefined).toBe(
          state.isStarting || state.isUpdating,
        );
      }
    }
    await wrapper.get('[aria-label="Play Sugar"]').trigger("click");
    expect(wrapper.emitted("playTrack")).toEqual([["track-3"]]);
    await wrapper
      .get('[aria-label="Remove Sugar from queue"]')
      .trigger("click");
    expect(wrapper.emitted("remove")).toEqual([[2]]);
    wrapper.unmount();
  });

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

  it("shows the active shuffled playback order", async () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: "track-1",
        isStarting: false,
        isUpdating: false,
        playbackOrder: ["track-1", "track-3", "track-2"],
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
        shuffleEnabled: true,
        status: "playing",
      },
    });

    expect(
      wrapper.get('[data-queue-item][data-queue-index="1"]').text(),
    ).toContain("Sugar");
    expect(
      wrapper.get('[data-queue-item][data-queue-index="2"]').text(),
    ).toContain("Toxicity");

    await wrapper
      .get('button[aria-label="Remove Toxicity from queue"]')
      .trigger("click");

    expect(wrapper.emitted("remove")).toEqual([[1]]);
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

  it("removes only an upcoming item through a visible queue action", async () => {
    const wrapper = mount(QueueWindow, {
      props: {
        currentItemId: "track-1",
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        queue,
        status: "playing",
      },
    });

    expect(
      wrapper.find('button[aria-label="Remove Aerials from queue"]').exists(),
    ).toBe(false);
    await wrapper
      .get('button[aria-label="Remove Toxicity from queue"]')
      .trigger("click");

    expect(wrapper.emitted("remove")).toEqual([[1]]);
  });

  it("opens queue context actions without offering current-item removal", async () => {
    const wrapper = mount(QueueWindow, {
      attachTo: document.body,
      props: {
        currentItemId: "track-1",
        isStarting: false,
        isUpdating: false,
        positionMs: 0,
        queue,
        status: "playing",
      },
    });

    await wrapper.get('[data-queue-item-id="track-1"]').trigger("contextmenu");
    expect(
      document.body.querySelector("[data-queue-context-menu]")?.textContent,
    ).not.toContain("Remove from queue");
    wrapper.unmount();
  });
});
