import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, ref } from "vue";
import { afterEach, describe, expect, it } from "vitest";

import type { MediaItem, PlaybackSnapshot, Playlist } from "@/api";
import ArtworkWindow from "../ArtworkWindow.vue";
import MiniPlayer from "../MiniPlayer.vue";
import QueueWindow from "../QueueWindow.vue";
import LibraryPlaybackFooter from "../library/LibraryPlaybackFooter.vue";

const tracks: MediaItem[] = [
  { id: "first", title: "First", artist: "Artist", durationMs: 1000 },
  { id: "second", title: "Second", artist: "Artist", durationMs: 1000 },
];
const snapshot: PlaybackSnapshot = {
  currentItem: tracks[0]!,
  queue: tracks,
  status: "paused",
  positionMs: 0,
  volumePercent: 50,
  shuffleEnabled: true,
  repeatMode: "one",
};
const cleanup: (() => void)[] = [];
afterEach(() => {
  cleanup.splice(0).forEach((dispose) => dispose());
  document.body.innerHTML = "";
});

describe("authoritative player controls", () => {
  it("renders footer favorites from current-track state and sends the selected ID", async () => {
    const selected = ref<MediaItem | null>(tracks[0]!);
    const favorites = ref(["first"]);
    const starting = ref(false);
    const wrapper = mount(
      defineComponent({
        setup: () => () =>
          h(LibraryPlaybackFooter, {
            playback: snapshot,
            currentItem: selected.value,
            isPlaying: false,
            isUpdating: false,
            detailsOpen: false,
            favoriteTrackIds: favorites.value,
            isStarting: starting.value,
            onToggleFavorite: (id: string) => {
              favorites.value = favorites.value.filter((item) => item !== id);
            },
          }),
      }),
    );
    cleanup.push(() => wrapper.unmount());
    expect(
      wrapper.get('[aria-label="Favorite track"]').attributes("aria-pressed"),
    ).toBe("true");
    await wrapper.get('[aria-label="Favorite track"]').trigger("click");
    expect(favorites.value).toEqual([]);
    selected.value = tracks[1]!;
    await flushPromises();
    expect(
      wrapper.get('[aria-label="Favorite track"]').attributes("aria-pressed"),
    ).toBe("false");
    selected.value = null;
    await flushPromises();
    expect(
      wrapper.get('[aria-label="Favorite track"]').attributes("disabled"),
    ).toBeDefined();
    selected.value = tracks[1]!;
    starting.value = true;
    await flushPromises();
    expect(
      wrapper.get('[aria-label="Disable shuffle"]').attributes("disabled"),
    ).toBeDefined();
    expect(
      wrapper.get('[aria-label="Disable repeat"]').attributes("disabled"),
    ).toBeDefined();
    expect(
      wrapper
        .get('[data-slot="slider"][aria-label="Track progress"]')
        .attributes("data-disabled"),
    ).toBeDefined();
  });

  it("connects artwork shuffle, repeat and favorite to authoritative parent state", async () => {
    const state = ref(snapshot);
    const favorites = ref(["first"]);
    const wrapper = mount(
      defineComponent({
        setup: () => () =>
          h(ArtworkWindow, {
            snapshot: state.value,
            isUpdating: false,
            favoriteTrackIds: favorites.value,
            onToggleShuffle: () => {
              state.value = { ...state.value, shuffleEnabled: false };
            },
            onCycleRepeatMode: () => {
              state.value = { ...state.value, repeatMode: "off" };
            },
            onToggleFavorite: (id: string) => {
              favorites.value = favorites.value.filter((item) => item !== id);
            },
          }),
      }),
      { attachTo: document.body },
    );
    cleanup.push(() => wrapper.unmount());
    expect(
      wrapper.get('[aria-label="Favorite track"]').attributes("aria-pressed"),
    ).toBe("true");
    await wrapper.get(".artwork-cover").trigger("contextmenu");
    expect(
      document.querySelector("[data-artwork-context-menu]")?.textContent,
    ).toContain("Remove from Favorites");
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await flushPromises();
    await wrapper.get('[aria-label="Disable shuffle"]').trigger("click");
    expect(
      wrapper.get('[aria-label="Enable shuffle"]').attributes("aria-pressed"),
    ).toBe("false");
    await wrapper.get('[aria-label="Disable repeat"]').trigger("click");
    expect(
      wrapper
        .get('[aria-label="Enable repeat all"]')
        .attributes("aria-pressed"),
    ).toBe("false");
    await wrapper.get('[aria-label="Favorite track"]').trigger("click");
    expect(favorites.value).toEqual([]);
  });

  it.each([ArtworkWindow, MiniPlayer])(
    "blocks duplicate transport actions while starting",
    (component) => {
      const wrapper = mount(component, {
        props: { snapshot, isUpdating: false, isStarting: true },
      });
      cleanup.push(() => wrapper.unmount());
      expect(
        wrapper.get('[aria-label="Starting playback"]').attributes("disabled"),
      ).toBeDefined();
      expect(
        wrapper.get('[aria-label="Previous track"]').attributes("disabled"),
      ).toBeDefined();
      expect(
        wrapper.get('[aria-label="Next track"]').attributes("disabled"),
      ).toBeDefined();
      expect(
        wrapper.get('[aria-label="Disable shuffle"]').attributes("disabled"),
      ).toBeDefined();
      expect(
        wrapper.get('[aria-label="Disable repeat"]').attributes("disabled"),
      ).toBeDefined();
      expect(
        wrapper
          .get('[data-slot="slider"][aria-label="Track progress"]')
          .attributes("data-disabled"),
      ).toBeDefined();
    },
  );
});

describe("queue actions", () => {
  function mountQueue(extra = {}) {
    const wrapper = mount(QueueWindow, {
      attachTo: document.body,
      props: {
        queue: tracks,
        currentItemId: "first",
        status: "playing",
        positionMs: 0,
        isStarting: false,
        isUpdating: false,
        ...extra,
      },
    });
    cleanup.push(() => wrapper.unmount());
    return wrapper;
  }
  const dialog = () => document.querySelector('[role="dialog"]') as HTMLElement;
  async function clickDialog(label: string) {
    const button = [...dialog().querySelectorAll("button")].find(
      (item) => item.textContent?.trim() === label,
    )!;
    button.click();
    await flushPromises();
  }

  it("confirms active clear, keeps failures visible and retries the same action", async () => {
    let attempts = 0;
    const wrapper = mountQueue({
      clearQueue: async () => {
        attempts += 1;
        if (attempts === 1) throw new Error("Audio unavailable");
        await wrapper.setProps({ queue: [], currentItemId: undefined });
      },
    });
    await wrapper.get('[aria-label="Clear queue"]').trigger("click");
    await flushPromises();
    expect(attempts).toBe(0);
    expect(dialog().textContent).toContain("stop playback");
    await clickDialog("Stop and clear queue");
    expect(dialog().textContent).toContain("Audio unavailable");
    await clickDialog("Stop and clear queue");
    expect(wrapper.text()).toContain("Your queue is empty");
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(
      wrapper.get('[aria-label="Clear queue"]').attributes("disabled"),
    ).toBeDefined();
    expect(
      wrapper
        .get('[aria-label="Save queue as playlist"]')
        .attributes("disabled"),
    ).toBeDefined();
  });

  it("preserves the name and ordered queue after failed save and returns focus after Escape", async () => {
    const saved: Playlist[] = [];
    let fail = true;
    const wrapper = mountQueue({
      shuffleEnabled: true,
      playbackOrder: ["second", "first"],
      savePlaylist: async (playlist: Playlist) => {
        if (fail) throw new Error("Disk is full");
        saved.push(playlist);
      },
    });
    const trigger = wrapper.get('[aria-label="Save queue as playlist"]');
    (trigger.element as HTMLButtonElement).focus();
    await trigger.trigger("click");
    await flushPromises();
    const input = dialog().querySelector("input")!;
    expect(document.activeElement).toBe(input);
    input.value = "My queue";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await flushPromises();
    await clickDialog("Save playlist");
    expect(dialog().textContent).toContain("Disk is full");
    expect(input.value).toBe("My queue");
    const saveButton = [...dialog().querySelectorAll("button")].find(
      (item) => item.textContent?.trim() === "Save playlist",
    )!;
    saveButton.focus();
    saveButton.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Tab",
        bubbles: true,
        cancelable: true,
      }),
    );
    await flushPromises();
    expect(document.activeElement).toBe(input);
    input.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Tab",
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    await flushPromises();
    expect(document.activeElement).toBe(saveButton);
    fail = false;
    await clickDialog("Save playlist");
    expect(saved[0]).toMatchObject({
      name: "My queue",
      trackIds: ["second", "first"],
    });
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    await trigger.trigger("click");
    await flushPromises();
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await flushPromises();
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(document.activeElement).toBe(trigger.element);
  });
});
