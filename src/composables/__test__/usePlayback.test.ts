import { describe, expect, it, vi } from "vitest";

import type { PlaybackSnapshot } from "@/api";

import { usePlayback } from "../usePlayback";

const paused: PlaybackSnapshot = {
  status: "paused",
  currentItem: null,
  positionMs: 0,
  volumePercent: 70,
  queue: [],
};

const playing: PlaybackSnapshot = { ...paused, status: "playing" };

describe("usePlayback", () => {
  it("loads a snapshot then uses play for a paused track", async () => {
    const client = {
      inspect: vi.fn().mockResolvedValue(paused),
      play: vi.fn().mockResolvedValue(playing),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn().mockResolvedValue(playing),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    await playback.toggle();

    expect(client.inspect).toHaveBeenCalledOnce();
    expect(client.play).toHaveBeenCalledOnce();
    expect(playback.snapshot.value).toEqual(playing);
  });

  it("marks a pending play command as starting", async () => {
    let resolvePlay: ((snapshot: PlaybackSnapshot) => void) | undefined;
    const pendingPlay = new Promise<PlaybackSnapshot>((resolve) => {
      resolvePlay = resolve;
    });
    const client = {
      inspect: vi.fn().mockResolvedValue(paused),
      play: vi.fn().mockReturnValue(pendingPlay),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn(),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    const starting = playback.toggle();

    expect(playback.isStarting.value).toBe(true);
    resolvePlay?.(playing);
    await starting;
    expect(playback.isStarting.value).toBe(false);
  });

  it("synchronizes advancing playback without entering the updating state", async () => {
    const advanced = { ...playing, positionMs: 2_000 };
    const client = {
      inspect: vi.fn().mockResolvedValue(advanced),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn(),
    };
    const playback = usePlayback(client);

    const synchronization = playback.sync();

    expect(playback.isUpdating.value).toBe(false);
    await synchronization;
    expect(playback.snapshot.value?.positionMs).toBe(2_000);
  });

  it("keeps library data stable while synchronizing transport state", async () => {
    const library = {
      tracks: [
        {
          artist: "YouTube Creators",
          durationMs: 207_000,
          id: "BaW_jenozKc",
          title: "Creator Studio Session",
        },
      ],
    };
    const initialTransport = {
      currentItem: library.tracks[0],
      positionMs: 0,
      status: "playing" as const,
      volumePercent: 70,
    };
    const advancedTransport = { ...initialTransport, positionMs: 2_000 };
    const client = {
      importYouTubeUrls: vi.fn(),
      inspect: vi.fn(),
      inspectLibrary: vi.fn().mockResolvedValue(library),
      inspectTransport: vi
        .fn()
        .mockResolvedValueOnce(initialTransport)
        .mockResolvedValueOnce(advancedTransport),
      moveQueueItem: vi.fn(),
      next: vi.fn(),
      pause: vi.fn(),
      play: vi.fn(),
      playTrack: vi.fn(),
      previous: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    const loadedLibrary = playback.library.value;
    await playback.sync();

    expect(playback.library.value).toBe(loadedLibrary);
    expect(playback.transport.value?.positionMs).toBe(2_000);
    expect(client.inspectLibrary).toHaveBeenCalledOnce();
    expect(client.inspectTransport).toHaveBeenCalledTimes(2);
  });

  it("starts import work without blocking playback commands", async () => {
    const client = {
      inspect: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn().mockResolvedValue(undefined),
    };
    const playback = usePlayback(client);
    const urls = [
      "https://www.youtube.com/playlist?list=PL-example",
      "https://www.youtube.com/@artist/videos",
    ];

    await playback.importYouTubeUrls(urls);

    expect(client.importYouTubeUrls).toHaveBeenCalledWith(urls);
    expect(playback.isUpdating.value).toBe(false);
    expect(playback.isImporting.value).toBe(true);

    playback.updateImportProgress({
      completedSources: 2,
      importedTracks: 7,
      message: "Imported 7 track(s) into the library.",
      phase: "completed",
      runId: 3,
      skippedMemberOnly: 0,
      totalSources: 2,
    });

    expect(playback.isImporting.value).toBe(false);
    expect(playback.importProgress.value?.importedTracks).toBe(7);
  });

  it("plays a selected library track", async () => {
    const selected = {
      ...playing,
      currentItem: {
        artist: "YouTube Creators",
        durationMs: 207_000,
        id: "BaW_jenozKc",
        title: "Creator Studio Session",
      },
    };
    const client = {
      inspect: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn().mockResolvedValue(selected),
      importYouTubeUrls: vi.fn(),
    };
    const playback = usePlayback(client);

    await playback.playTrack("BaW_jenozKc");

    expect(client.playTrack).toHaveBeenCalledWith("BaW_jenozKc");
    expect(playback.snapshot.value).toEqual(selected);
  });

  it("replaces the library after saving edited metadata without changing transport", async () => {
    const library = {
      tracks: [
        {
          album: "API Sessions",
          artist: "Google for Developers",
          durationMs: 238_000,
          id: "M7lc1UVf-VE",
          title: "YouTube Developers Live",
        },
      ],
    };
    const updatedLibrary = {
      tracks: [{ ...library.tracks[0], title: "Renamed session" }],
    };
    const transport = {
      currentItem: library.tracks[0],
      positionMs: 4_000,
      status: "playing" as const,
      volumePercent: 70,
    };
    const client = {
      importYouTubeUrls: vi.fn(),
      inspect: vi.fn(),
      inspectLibrary: vi.fn().mockResolvedValue(library),
      inspectTransport: vi.fn().mockResolvedValue(transport),
      moveQueueItem: vi.fn(),
      next: vi.fn(),
      pause: vi.fn(),
      play: vi.fn(),
      playTrack: vi.fn(),
      previous: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      updateTracksMetadata: vi.fn().mockResolvedValue(updatedLibrary),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    await playback.updateTracksMetadata([
      {
        id: "M7lc1UVf-VE",
        metadata: {
          album: "API Sessions",
          artist: "Google for Developers",
          genres: [],
          label: null,
          title: "Renamed session",
        },
      },
    ]);

    expect(client.updateTracksMetadata).toHaveBeenCalledOnce();
    expect(playback.library.value).toEqual(updatedLibrary);
    expect(playback.transport.value).toEqual(transport);
  });

  it("mutes and restores the prior non-zero volume", async () => {
    const muted = { ...paused, volumePercent: 0 };
    const restored = { ...paused, volumePercent: 70 };
    const client = {
      importYouTubeUrls: vi.fn(),
      inspect: vi.fn().mockResolvedValue(paused),
      moveQueueItem: vi.fn(),
      next: vi.fn(),
      pause: vi.fn(),
      play: vi.fn(),
      playTrack: vi.fn(),
      previous: vi.fn(),
      seek: vi.fn(),
      setVolume: vi
        .fn()
        .mockResolvedValueOnce(muted)
        .mockResolvedValueOnce(restored),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    await playback.toggleMute();
    await playback.toggleMute();

    expect(client.setVolume).toHaveBeenNthCalledWith(1, 0);
    expect(client.setVolume).toHaveBeenNthCalledWith(2, 70);
    expect(playback.snapshot.value?.volumePercent).toBe(70);
  });

  it("sends the latest volume reached during a drag", async () => {
    let resolveInitialVolume: ((value: PlaybackSnapshot) => void) | undefined;
    const initialVolume = new Promise<PlaybackSnapshot>((resolve) => {
      resolveInitialVolume = resolve;
    });
    const client = {
      importYouTubeUrls: vi.fn(),
      inspect: vi.fn().mockResolvedValue(paused),
      moveQueueItem: vi.fn(),
      next: vi.fn(),
      pause: vi.fn(),
      play: vi.fn(),
      playTrack: vi.fn(),
      previous: vi.fn(),
      seek: vi.fn(),
      setVolume: vi
        .fn()
        .mockReturnValueOnce(initialVolume)
        .mockResolvedValueOnce({ ...paused, volumePercent: 50 }),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    const firstUpdate = playback.setVolume(20);
    const latestUpdate = playback.setVolume(50);
    resolveInitialVolume?.({ ...paused, volumePercent: 20 });
    await Promise.all([firstUpdate, latestUpdate]);

    expect(client.setVolume).toHaveBeenNthCalledWith(1, 20);
    expect(client.setVolume).toHaveBeenNthCalledWith(2, 50);
    expect(playback.isUpdating.value).toBe(false);
    expect(playback.snapshot.value?.volumePercent).toBe(50);
  });

  it("updates volume locally without replacing the library during a drag", async () => {
    const initial = {
      ...paused,
      queue: [
        {
          artist: "YouTube Creators",
          durationMs: 207_000,
          id: "BaW_jenozKc",
          title: "Creator Studio Session",
        },
      ],
    };
    let resolveVolume: ((snapshot: PlaybackSnapshot) => void) | undefined;
    const pendingVolume = new Promise<PlaybackSnapshot>((resolve) => {
      resolveVolume = resolve;
    });
    const client = {
      importYouTubeUrls: vi.fn(),
      inspect: vi.fn().mockResolvedValue(initial),
      moveQueueItem: vi.fn(),
      next: vi.fn(),
      pause: vi.fn(),
      play: vi.fn(),
      playTrack: vi.fn(),
      previous: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn().mockReturnValue(pendingVolume),
    };
    const playback = usePlayback(client);

    await playback.refresh();
    const loadedLibrary = playback.library.value;
    const update = playback.setVolume(45);

    expect(playback.snapshot.value?.volumePercent).toBe(45);
    expect(playback.library.value).toBe(loadedLibrary);

    resolveVolume?.({
      ...initial,
      volumePercent: 45,
      queue: [...initial.queue],
    });
    await update;

    expect(playback.library.value).toBe(loadedLibrary);
  });

  it("preserves a structured Tauri command error message", async () => {
    const client = {
      inspect: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      previous: vi.fn(),
      next: vi.fn(),
      seek: vi.fn(),
      setVolume: vi.fn(),
      moveQueueItem: vi.fn(),
      playTrack: vi.fn(),
      importYouTubeUrls: vi.fn().mockRejectedValue({
        code: "youtube_metadata_failed",
        message: "could not resolve YouTube metadata",
      }),
    };
    const playback = usePlayback(client);

    await playback.importYouTubeUrls(["https://youtu.be/wEsuJoBKAvA"]);

    expect(playback.errorMessage.value).toBe(
      "could not resolve YouTube metadata",
    );
  });
});
