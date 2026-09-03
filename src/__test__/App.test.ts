import { getCurrentWindow } from "@tauri-apps/api/window";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";

import App from "../App.vue";
import LibraryWindow from "../components/LibraryWindow.vue";
import QueueWindow from "../components/QueueWindow.vue";

const playbackMocks = vi.hoisted(() => ({
  applySnapshot: vi.fn(),
  importYouTubeUrls: vi.fn(),
  moveQueueItem: vi.fn(),
  reorderPlaylists: vi.fn(),
  removeQueueItem: vi.fn(),
  removeTracks: vi.fn(),
  next: vi.fn(),
  playTrack: vi.fn(),
  previous: vi.fn(),
  refresh: vi.fn(),
  sync: vi.fn(),
  toggle: vi.fn(),
  toggleMute: vi.fn(),
}));
const windowMocks = vi.hoisted(() => ({
  showImport: vi.fn(),
}));
const eventMocks = vi.hoisted(() => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(),
  LogicalSize: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => eventMocks);

vi.mock("@/api", async (importOriginal) => {
  const api = await importOriginal<typeof import("@/api")>();

  return {
    ...api,
    windowApi: windowMocks,
  };
});

vi.mock("@/composables/usePlayback", () => ({
  usePlayback: () => ({
    applySnapshot: playbackMocks.applySnapshot,
    snapshot: {
      value: {
        status: "paused",
        currentItem: {
          id: "night-drive",
          title: "Night Drive",
          artist: "Chromatic Skies",
          durationMs: 238_000,
        },
        positionMs: 57_000,
        volumePercent: 72,
        queue: [
          {
            id: "night-drive",
            title: "Night Drive",
            artist: "Chromatic Skies",
            durationMs: 238_000,
          },
        ],
      },
    },
    library: {
      value: {
        tracks: [
          {
            id: "night-drive",
            title: "Night Drive",
            artist: "Chromatic Skies",
            durationMs: 238_000,
          },
        ],
      },
    },
    transport: {
      value: {
        status: "paused",
        currentItem: {
          id: "night-drive",
          title: "Night Drive",
          artist: "Chromatic Skies",
          durationMs: 238_000,
        },
        positionMs: 57_000,
        volumePercent: 72,
      },
    },
    isStarting: { value: false },
    isUpdating: { value: false },
    isImporting: { value: false },
    importProgress: { value: null },
    metadataRefreshes: {
      value: { completedTracks: 0, jobs: [], totalTracks: 0 },
    },
    errorMessage: { value: "" },
    refresh: playbackMocks.refresh,
    sync: playbackMocks.sync,
    toggle: playbackMocks.toggle,
    toggleMute: playbackMocks.toggleMute,
    previous: playbackMocks.previous,
    next: playbackMocks.next,
    seek: vi.fn(),
    setVolume: vi.fn(),
    moveQueueItem: playbackMocks.moveQueueItem,
    reorderPlaylists: playbackMocks.reorderPlaylists,
    removeQueueItem: playbackMocks.removeQueueItem,
    removeTracks: playbackMocks.removeTracks,
    playTrack: playbackMocks.playTrack,
    importYouTubeUrls: playbackMocks.importYouTubeUrls,
    updateImportProgress: vi.fn(),
    updateMetadataRefreshes: vi.fn(),
  }),
}));

describe("application landmarks", () => {
  beforeEach(() => {
    playbackMocks.applySnapshot.mockReset();
    playbackMocks.importYouTubeUrls.mockReset();
    playbackMocks.moveQueueItem.mockReset();
    playbackMocks.reorderPlaylists.mockReset();
    playbackMocks.removeQueueItem.mockReset();
    playbackMocks.removeTracks.mockReset();
    playbackMocks.next.mockReset();
    playbackMocks.playTrack.mockReset();
    playbackMocks.previous.mockReset();
    playbackMocks.refresh.mockReset();
    playbackMocks.sync.mockReset();
    playbackMocks.toggle.mockReset();
    playbackMocks.toggleMute.mockReset();
    windowMocks.showImport.mockReset();
    eventMocks.listen.mockReset();
    eventMocks.listen.mockResolvedValue(vi.fn());
    window.history.replaceState({}, "", "/?view=library");
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.useRealTimers();
    document.documentElement.removeAttribute("data-theme");
  });

  it("renders exactly one main landmark for the active native window", async () => {
    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.findAll("main")).toHaveLength(1);
    expect(wrapper.get("main").attributes("aria-label")).toBe("Music library");
  });

  it("passes library and transport state separately to the library window", async () => {
    const wrapper = mount(App);
    await flushPromises();
    const libraryWindow = wrapper.getComponent(LibraryWindow);

    expect(libraryWindow.props("tracks")).toHaveLength(1);
    expect(libraryWindow.props("transport")).toMatchObject({
      positionMs: 57_000,
      status: "paused",
    });
  });

  it("routes reordered playlist titles to the playback composable", async () => {
    const wrapper = mount(App);
    await flushPromises();

    wrapper
      .getComponent(LibraryWindow)
      .vm.$emit("reorderPlaylists", ["road-trip", "focus"]);

    expect(playbackMocks.reorderPlaylists).toHaveBeenCalledWith([
      "road-trip",
      "focus",
    ]);
  });

  it("opens Import Music from the Library plus button", async () => {
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('button[aria-label="Import music"]').trigger("click");

    expect(windowMocks.showImport).toHaveBeenCalledOnce();
  });

  it("routes a multi-source Import window submission to playback", async () => {
    window.history.replaceState({}, "", "/?view=import");
    const wrapper = mount(App);
    await flushPromises();
    const form = wrapper.get('form[aria-label="Import music from YouTube"]');

    await form
      .get("textarea")
      .setValue(
        "https://youtu.be/wEsuJoBKAvA\nhttps://youtube.com/@artist/videos",
      );
    await form.trigger("submit");

    expect(playbackMocks.importYouTubeUrls).toHaveBeenCalledWith([
      "https://youtu.be/wEsuJoBKAvA",
      "https://youtube.com/@artist/videos",
    ]);
  });

  it("routes a double-clicked Library track to the playback composable", async () => {
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('[data-track-id="night-drive"]').trigger("dblclick");

    expect(playbackMocks.playTrack).toHaveBeenCalledWith("night-drive", [
      "night-drive",
    ]);
  });

  it("routes queue playback and reorder actions to the playback composable", async () => {
    window.history.replaceState({}, "", "/?view=queue");
    const wrapper = mount(App);
    await flushPromises();

    await wrapper.get('button[aria-label="Play Night Drive"]').trigger("click");
    wrapper.getComponent(QueueWindow).vm.$emit("move", 0, 1);

    expect(wrapper.getComponent(QueueWindow).props("status")).toBe("paused");
    expect(playbackMocks.playTrack).toHaveBeenCalledWith("night-drive");
    expect(playbackMocks.moveQueueItem).toHaveBeenCalledWith(0, 1);
  });

  it("routes library and queue removal actions to their separate playback commands", async () => {
    const libraryWrapper = mount(App);
    await flushPromises();

    libraryWrapper
      .getComponent(LibraryWindow)
      .vm.$emit("removeTracks", ["night-drive"]);
    expect(playbackMocks.removeTracks).toHaveBeenCalledWith(["night-drive"]);
    libraryWrapper.unmount();

    window.history.replaceState({}, "", "/?view=queue");
    const queueWrapper = mount(App);
    await flushPromises();

    queueWrapper.getComponent(QueueWindow).vm.$emit("remove", 0);
    expect(playbackMocks.removeQueueItem).toHaveBeenCalledWith(0);
    queueWrapper.unmount();
  });

  it("synchronizes live playback while the window is mounted", async () => {
    vi.useFakeTimers();
    const wrapper = mount(App);
    await flushPromises();

    await vi.advanceTimersByTimeAsync(500);

    expect(playbackMocks.sync).toHaveBeenCalledOnce();
    wrapper.unmount();
  });

  it("routes keyboard playback controls outside editable fields", async () => {
    const wrapper = mount(App);
    await flushPromises();

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "j" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "m" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: " " }));

    expect(playbackMocks.previous).toHaveBeenCalled();
    expect(playbackMocks.next).toHaveBeenCalled();
    expect(playbackMocks.toggleMute).toHaveBeenCalled();
    expect(playbackMocks.toggle).toHaveBeenCalled();
    wrapper.unmount();
  });

  it("applies native menu playback updates and opens keyboard shortcuts", async () => {
    let playbackUpdated: ((event: { payload: unknown }) => void) | undefined;
    let showKeyboardShortcuts: (() => void) | undefined;
    eventMocks.listen.mockImplementation(async (event, handler) => {
      if (event === "playback-updated") {
        playbackUpdated = handler as (event: { payload: unknown }) => void;
      }
      if (event === "show-keyboard-shortcuts") {
        showKeyboardShortcuts = handler as () => void;
      }
      return vi.fn();
    });
    const wrapper = mount(App);
    await flushPromises();

    playbackUpdated?.({
      payload: {
        currentItem: null,
        positionMs: 0,
        queue: [],
        status: "playing",
        volumePercent: 50,
      },
    });
    showKeyboardShortcuts?.();
    await nextTick();

    expect(playbackMocks.applySnapshot).toHaveBeenCalled();
    expect(wrapper.get('[role="dialog"]').text()).toContain(
      "Keyboard Shortcuts",
    );
    expect(wrapper.get('[role="dialog"]').classes()).toContain("bg-black/60");
    expect(wrapper.get('[role="dialog"] > div').classes()).toContain(
      "bg-[oklch(0.11_0.014_260/0.98)]",
    );

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await nextTick();
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it("refreshes the library after a native metadata update", async () => {
    let libraryUpdated: (() => void) | undefined;
    eventMocks.listen.mockImplementation(async (event, handler) => {
      if (event === "library-updated") {
        libraryUpdated = handler as () => void;
      }
      return vi.fn();
    });
    const wrapper = mount(App);
    await flushPromises();
    playbackMocks.refresh.mockClear();

    expect(libraryUpdated).toBeDefined();
    libraryUpdated?.();
    await flushPromises();

    expect(playbackMocks.refresh).toHaveBeenCalledOnce();
    wrapper.unmount();
  });

  it("renders the settings window as one named main landmark", async () => {
    window.history.replaceState({}, "", "/?view=settings");

    const wrapper = mount(App);
    await flushPromises();

    expect(wrapper.findAll("main")).toHaveLength(1);
    expect(wrapper.get("main").attributes("aria-label")).toBe("Settings");
  });

  it("tracks native focus for the Artwork controls", async () => {
    let focusListener: ((event: { payload: boolean }) => void) | undefined;
    const unlisten = vi.fn();
    vi.mocked(getCurrentWindow).mockReturnValue({
      isFocused: vi.fn().mockResolvedValue(true),
      onFocusChanged: vi.fn(
        async (listener: (event: { payload: boolean }) => void) => {
          focusListener = listener;
          return unlisten;
        },
      ),
    } as never);
    window.history.replaceState({}, "", "/?view=artwork");

    const wrapper = mount(App);
    await flushPromises();
    const controls = wrapper.get(".artwork-playback-controls");
    expect(controls.attributes("data-window-focused")).toBe("true");

    expect(focusListener).toBeDefined();
    focusListener!({ payload: false });
    await nextTick();

    expect(controls.attributes("data-window-focused")).toBe("false");

    wrapper.unmount();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
