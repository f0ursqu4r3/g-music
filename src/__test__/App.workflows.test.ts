import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Storage } from "happy-dom";
import { DOMWrapper, mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushApp as flushPromises } from "./flushApp";

import type {
  ImportProgress,
  LibrarySnapshot,
  MetadataRefreshSnapshot,
  PlaybackSnapshot,
  Playlist,
} from "@/api";
import App from "../App.vue";

const events = vi.hoisted(
  () => new Map<string, (event: { payload: unknown }) => void>(),
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: (path: string) => path,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name, handler) => {
    events.set(name, handler);
    return () => events.delete(name);
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  LogicalSize: vi.fn(),
  getCurrentWindow: () => ({
    isFocused: async () => true,
    onFocusChanged: async () => () => {},
    close: async () => {},
  }),
}));

const first = {
  id: "one",
  title: "First session",
  artist: "Artist one",
  durationMs: 120000,
};
const second = {
  id: "two",
  title: "Second session",
  artist: "Artist two",
  durationMs: 130000,
};
const discovery = {
  id: "found",
  title: "Discovered recording",
  artist: "Search artist",
  album: "Search album",
  durationMs: 145000,
  sourceUrl: "https://www.youtube.com/watch?v=found",
};
let state: PlaybackSnapshot;
let library: LibrarySnapshot;
let metadata: MetadataRefreshSnapshot;
let progress: ImportProgress;
let wrapper: VueWrapper | undefined;
const commandOverrides = new Map<string, () => Promise<unknown>>();

async function open(view: string): Promise<VueWrapper> {
  window.history.replaceState({}, "", `/?view=${view}`);
  wrapper = mount(App, { attachTo: document.body });
  await flushPromises();
  return wrapper;
}

function dialog(): DOMWrapper<Element> {
  const element = document.querySelector('[role="dialog"][data-state="open"]');
  expect(element).not.toBeNull();
  return new DOMWrapper(element!);
}

beforeEach(() => {
  vi.stubGlobal("localStorage", new Storage());
  events.clear();
  commandOverrides.clear();
  window.localStorage.clear();
  state = {
    status: "playing",
    currentItem: first,
    queue: [first, second],
    positionMs: 1000,
    volumePercent: 65,
    shuffleEnabled: false,
    repeatMode: "off",
  };
  library = { tracks: [first, second], playlists: [] };
  metadata = { jobs: [], totalTracks: 0, completedTracks: 0 };
  progress = {
    completedSources: 0,
    importedTracks: 2,
    message: "Reading source",
    phase: "resolving",
    runId: 84,
    skippedMemberOnly: 0,
    totalSources: 1,
  };
  vi.mocked(invoke)
    .mockReset()
    .mockImplementation(async (command, args) => {
      const override = commandOverrides.get(command);
      if (override) return override();
      switch (command) {
        case "inspect_library":
          return library;
        case "inspect_metadata_refreshes":
          return metadata;
        case "inspect_import_progress":
          return null;
        case "inspect_youtube_auth":
          return { connected: false };
        case "inspect_diagnostics":
          return {
            appVersion: "0.2.0",
            platform: "macos",
            dependencies: [
              {
                name: "mpv",
                available: false,
                version: null,
                message: "Install mpv and refresh diagnostics.",
              },
            ],
            audioOutputPolicy: "System default output",
          };
        case "search_youtube":
          return [discovery];
        case "import_youtube_urls":
          events.get("import-progress")?.({ payload: progress });
          library = { ...library, tracks: [...library.tracks, discovery] };
          return undefined;
        case "cancel_youtube_import":
          progress = {
            ...progress,
            phase: "cancelled",
            message: "Import cancelled",
          };
          events.get("import-progress")?.({ payload: progress });
          return undefined;
        case "retry_metadata_refreshes":
          metadata = {
            ...metadata,
            jobs: metadata.jobs.map((job) => ({
              ...job,
              state: "queued",
              message: "Queued for retry",
            })),
          };
          return metadata;
        case "export_library_backup":
          return { path: "/private/backups/library.sqlite3" };
        case "clear_queue":
          state = {
            ...state,
            queue: [],
            currentItem: null,
            status: "paused",
            positionMs: 0,
          };
          return state;
        case "upsert_playlist":
          library = {
            ...library,
            playlists: [
              ...library.playlists,
              (args as { playlist: Playlist }).playlist,
            ],
          };
          return library;
        case "resolve_youtube_artwork":
          return null;
        default:
          return state;
      }
    });
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  document.body.innerHTML = "";
});

describe("App product workflows through real components and IPC", () => {
  it.each(["library", "mini", "queue", "artwork", "settings"])(
    "reconnects an expired YouTube session from the %s error notification",
    async (view) => {
      const app = await open(view);
      const message =
        "Your saved YouTube session may have expired. Sign in again, save the session, then retry playback.";
      events.get("playback-error")?.({
        payload: { code: "youtube_session_expired", message },
      });
      await flushPromises();
      const action = () => app.get("[data-sonner-toast] button[data-action]");
      expect(action().text()).toBe("Reconnect YouTube");
      await action().trigger("click");
      await flushPromises();
      expect(invoke).toHaveBeenCalledWith("open_youtube_login");
      expect(invoke).not.toHaveBeenCalledWith("disconnect_youtube");
      expect(action().text()).toBe("Save session and retry");
      expect(app.text()).toContain("Complete sign-in in the YouTube window");
      await action().trigger("click");
      await flushPromises();
      expect(invoke).toHaveBeenCalledWith("save_youtube_session");
      expect(invoke).toHaveBeenCalledWith("play");
    },
  );

  it("shows a startup string error once and retries the failed inspection", async () => {
    const message = "Command inspect_import_progress not found";
    commandOverrides.set("inspect_import_progress", () =>
      Promise.reject(message),
    );
    const app = await open("library");
    expect(app.text().split(message)).toHaveLength(2);
    expect(
      app.findAll('[data-sonner-toast][data-removed="false"]'),
    ).toHaveLength(1);
    expect(app.text()).not.toContain("The playback command failed.");

    commandOverrides.set("inspect_import_progress", async () => null);
    await app.get("[data-sonner-toast] button[data-action]").trigger("click");
    await flushPromises();
    await expect
      .poll(() =>
        app.find('[data-sonner-toast][data-removed="false"]').exists(),
      )
      .toBe(false);
    expect(app.text()).toContain("First session");
  });

  it("shows a structured playback error only once in the library", async () => {
    const app = await open("library");
    const message = "The audio output is unavailable.";
    events.get("playback-error")?.({ payload: { code: "player", message } });
    await flushPromises();
    expect(app.text().split(message)).toHaveLength(2);
    expect(app.get("[data-sonner-toast] button[data-action]").text()).toBe(
      "Retry",
    );
    await app.get('button[aria-label="Dismiss error"]').trigger("click");
    await expect.poll(() => app.text()).not.toContain(message);
  });

  it("keeps the reconnect notification after a failed save and blocks duplicate saves", async () => {
    const app = await open("library");
    events.get("playback-error")?.({
      payload: {
        code: "youtube_session_expired",
        message: "Sign in again to restore your YouTube session.",
      },
    });
    await flushPromises();
    const action = () => app.get("[data-sonner-toast] button[data-action]");
    await action().trigger("click");
    await flushPromises();
    let reject!: (reason: unknown) => void;
    commandOverrides.set(
      "save_youtube_session",
      () =>
        new Promise((_, fail) => {
          reject = fail;
        }),
    );
    await action().trigger("click");
    await action().trigger("click");
    expect(
      vi
        .mocked(invoke)
        .mock.calls.filter(([command]) => command === "save_youtube_session"),
    ).toHaveLength(1);
    reject({ message: "Finish signing in before saving the session." });
    await flushPromises();
    expect(app.text()).toContain(
      "Finish signing in before saving the session.",
    );
    expect(action().text()).toBe("Save session and retry");
    expect(invoke).not.toHaveBeenCalledWith("play");
    commandOverrides.set("save_youtube_session", async () => ({
      connected: true,
    }));
    await action().trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("play");
  });

  it("updates one Sonner toast when the same playback error repeats", async () => {
    const app = await open("library");
    for (const message of [
      "Player unavailable",
      "Player unavailable",
      "Output disconnected",
    ]) {
      events.get("playback-error")?.({ payload: { code: "player", message } });
      await flushPromises();
    }
    const toasts = app.findAll('[data-sonner-toast][data-removed="false"]');
    expect(toasts).toHaveLength(1);
    expect(toasts[0].attributes("data-type")).toBe("error");
    expect(toasts[0].text()).toContain("Output disconnected");
    expect(app.find('[role="alert"]').exists()).toBe(false);
  });

  it("retains import progress received while other subscriptions are pending", async () => {
    let release!: () => void;
    vi.mocked(listen).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = () => resolve(() => {});
        }),
    );
    const app = await open("import");
    events.get("import-progress")?.({ payload: progress });
    expect(invoke).not.toHaveBeenCalledWith("inspect_import_progress");
    release();
    await flushPromises();
    expect(app.get('[role="log"]').text()).toContain("Reading source");
    await app.get('button[aria-label="Cancel import"]').trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("cancel_youtube_import", { runId: 84 });
  });

  it.each(["import", "library"])(
    "shows and cancels an existing import in a newly opened %s window",
    async (view) => {
      commandOverrides.set("inspect_import_progress", async () => {
        expect(events.has("import-progress")).toBe(true);
        return progress;
      });
      const app = await open(view);
      expect(invoke).toHaveBeenCalledWith("inspect_import_progress");
      expect(app.get('[aria-label="Import progress"]').text()).toContain(
        "Reading source",
      );
      await app.get('button[aria-label="Cancel import"]').trigger("click");
      await flushPromises();
      expect(invoke).toHaveBeenCalledWith("cancel_youtube_import", {
        runId: 84,
      });
      expect(app.find('button[aria-label="Cancel import"]').exists()).toBe(
        false,
      );
    },
  );

  it.each([null, "stale"])(
    "keeps a newer import event when the initial inspection returns %s",
    async (result) => {
      let finish!: (value: ImportProgress | null) => void;
      commandOverrides.set(
        "inspect_import_progress",
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      );
      const app = await open("import");
      events.get("import-progress")?.({ payload: progress });
      await flushPromises();
      expect(invoke).toHaveBeenCalledWith("inspect_import_progress");
      finish(
        result === null ? null : { ...progress, runId: 83, message: "Old run" },
      );
      await flushPromises();
      expect(app.get('[role="log"]').text()).toContain("Reading source");
      expect(app.get('[role="log"]').text()).not.toContain("Old run");
      await app.get('button[aria-label="Cancel import"]').trigger("click");
      await flushPromises();
      expect(invoke).toHaveBeenCalledWith("cancel_youtube_import", {
        runId: 84,
      });
    },
  );

  it("searches text, shows provider metadata, and imports only after an explicit action", async () => {
    const app = await open("import");
    await app
      .get('input[aria-label="Search YouTube"]')
      .setValue("  artist recording  ");
    await app.get('form[aria-label="Search YouTube"]').trigger("submit");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("search_youtube", {
      query: "artist recording",
    });
    expect(app.get('[data-search-result="found"]').text()).toContain(
      "Search artist",
    );
    expect(app.get('[data-search-result="found"]').text()).toContain(
      "Search album",
    );
    expect(
      vi
        .mocked(invoke)
        .mock.calls.some(
          ([command]) =>
            command === "import_youtube_urls" || command === "play_track",
        ),
    ).toBe(false);
    await app
      .get('button[aria-label="Import Discovered recording"]')
      .trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("import_youtube_urls", {
      urls: [discovery.sourceUrl],
    });
    expect(app.get('[role="log"]').text()).toContain("Reading source");
    expect(
      app
        .get('button[aria-label="Import Discovered recording"]')
        .attributes("disabled"),
    ).toBeDefined();
  });

  it("cancels the native active run and retains committed-track messaging", async () => {
    const app = await open("import");
    events.get("import-progress")?.({ payload: progress });
    await flushPromises();
    await app.get('button[aria-label="Cancel import"]').trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("cancel_youtube_import", { runId: 84 });
    expect(app.text()).toContain("2 tracks remain in your library");
    expect(app.find('button[aria-label="Cancel import"]').exists()).toBe(false);
  });

  it("blocks duplicate cancellation while the backend is cancelling", async () => {
    let finish!: () => void;
    commandOverrides.set(
      "cancel_youtube_import",
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const app = await open("import");
    events.get("import-progress")?.({ payload: progress });
    await flushPromises();
    await app.get('button[aria-label="Cancel import"]').trigger("click");
    expect(
      app.get('button[aria-label="Cancel import"]').attributes("disabled"),
    ).toBeDefined();
    await app.get('button[aria-label="Cancel import"]').trigger("click");
    expect(
      vi
        .mocked(invoke)
        .mock.calls.filter(([command]) => command === "cancel_youtube_import"),
    ).toHaveLength(1);
    finish();
    await flushPromises();
  });

  it("retries failed enrichment through the real composable and updates the visible state", async () => {
    metadata = {
      totalTracks: 1,
      completedTracks: 0,
      jobs: [
        {
          trackId: first.id,
          title: first.title,
          state: "failed",
          message: "Provider unavailable",
        },
      ],
    };
    const app = await open("import");
    await app
      .get('button[aria-label="Retry failed metadata"]')
      .trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("retry_metadata_refreshes");
    expect(
      app.find('button[aria-label="Retry failed metadata"]').exists(),
    ).toBe(false);
    expect(app.get('[aria-label="Metadata refresh"]').text()).toContain(
      "0 of 1 refreshed",
    );
  });

  it("keeps a failed URL import draft and retries the same source through IPC", async () => {
    commandOverrides.set("import_youtube_urls", async () => {
      commandOverrides.delete("import_youtube_urls");
      throw {
        code: "import_failed",
        message: "Import failed. Check your connection.",
      };
    });
    const app = await open("import");
    const input = app.get('textarea[aria-label="YouTube URLs"]');
    await input.setValue("https://youtu.be/retain-this");
    await app
      .get('form[aria-label="Import music from YouTube"]')
      .trigger("submit");
    await flushPromises();
    expect(app.text()).toContain("Import failed. Check your connection.");
    expect((input.element as HTMLTextAreaElement).value).toBe(
      "https://youtu.be/retain-this",
    );
    await app.get("[data-sonner-toast] button[data-action]").trigger("click");
    await flushPromises();
    expect(
      vi
        .mocked(invoke)
        .mock.calls.filter(([command]) => command === "import_youtube_urls"),
    ).toEqual([
      ["import_youtube_urls", { urls: ["https://youtu.be/retain-this"] }],
      ["import_youtube_urls", { urls: ["https://youtu.be/retain-this"] }],
    ]);
    await expect
      .poll(() =>
        app.find('[data-sonner-toast][data-removed="false"]').exists(),
      )
      .toBe(false);
  });

  it("persists Settings theme, refreshes diagnostics, and creates a selectable backup path", async () => {
    const app = await open("settings");
    expect(app.text()).toContain("Install mpv and refresh diagnostics");
    await app.get('select[aria-label="Theme"]').setValue("ember");
    await flushPromises();
    expect(document.documentElement.dataset.theme).toBe("ember");
    expect(window.localStorage.getItem("gmusic-theme")).toBe("ember");
    await app.get('button[aria-label="Refresh diagnostics"]').trigger("click");
    await flushPromises();
    expect(
      vi
        .mocked(invoke)
        .mock.calls.filter(([command]) => command === "inspect_diagnostics"),
    ).toHaveLength(2);
    await app.get('button[aria-label="Back up library"]').trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("export_library_backup");
    expect(
      (app.get('input[aria-label="Backup path"]').element as HTMLInputElement)
        .value,
    ).toBe("/private/backups/library.sqlite3");
    expect(app.text()).toContain("Library backup created");
  });

  it("requires active queue confirmation and applies the stopped empty snapshot", async () => {
    const app = await open("queue");
    await app.get('button[aria-label="Clear queue"]').trigger("click");
    await flushPromises();
    expect(invoke).not.toHaveBeenCalledWith("clear_queue");
    expect(dialog().text()).toContain("This will stop playback");
    await dialog()
      .findAll("button")
      .find((button) => button.text() === "Stop and clear queue")!
      .trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("clear_queue");
    expect(app.text()).toContain("Your queue is empty");
    expect(app.find("[data-queue-footer]").exists()).toBe(false);
    expect(
      app.get('button[aria-label="Clear queue"]').attributes("disabled"),
    ).toBeDefined();
  });

  it("saves the authoritative queue playback order through a named playlist dialog", async () => {
    state = {
      ...state,
      shuffleEnabled: true,
      playbackOrder: [second.id, first.id],
      currentItem: second,
    };
    const app = await open("queue");
    await app
      .get('button[aria-label="Save queue as playlist"]')
      .trigger("click");
    await flushPromises();
    await dialog().get("input").setValue("  Working queue  ");
    await dialog().get("form").trigger("submit");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("upsert_playlist", {
      playlist: {
        id: expect.stringMatching(/^playlist-/),
        name: "Working queue",
        trackIds: [second.id, first.id],
      },
    });
    expect(
      document.querySelector('[role="dialog"][data-state="open"]'),
    ).toBeNull();
    expect(app.text()).toContain("2 tracks");
  });

  it("keeps a failed queue playlist draft and retries the same ordered playlist", async () => {
    commandOverrides.set("upsert_playlist", async () => {
      commandOverrides.delete("upsert_playlist");
      throw { code: "save_failed", message: "Disk is full" };
    });
    const app = await open("queue");
    await app
      .get('button[aria-label="Save queue as playlist"]')
      .trigger("click");
    await flushPromises();
    await dialog().get("input").setValue("Keep my draft");
    await dialog().get("form").trigger("submit");
    await flushPromises();
    expect(dialog().get('[role="alert"]').text()).toContain("Disk is full");
    expect((dialog().get("input").element as HTMLInputElement).value).toBe(
      "Keep my draft",
    );
    const firstSave = vi
      .mocked(invoke)
      .mock.calls.find(([command]) => command === "upsert_playlist");
    await dialog().get("form").trigger("submit");
    await flushPromises();
    const saves = vi
      .mocked(invoke)
      .mock.calls.filter(([command]) => command === "upsert_playlist");
    expect(saves).toEqual([firstSave, firstSave]);
    expect(
      document.querySelector('[role="dialog"][data-state="open"]'),
    ).toBeNull();
  });
});
