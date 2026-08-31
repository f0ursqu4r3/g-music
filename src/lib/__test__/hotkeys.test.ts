import { describe, expect, it } from "vitest";

import { resolvePlaybackHotkey } from "../hotkeys";

describe("playback hotkeys", () => {
  it("maps unmodified playback keys to their actions", () => {
    expect(resolvePlaybackHotkey(" ", false)).toBe("toggle");
    expect(resolvePlaybackHotkey("j", false)).toBe("previous");
    expect(resolvePlaybackHotkey("K", false)).toBe("next");
    expect(resolvePlaybackHotkey("m", false)).toBe("toggleMute");
  });

  it("keeps typing and modified shortcuts under browser control", () => {
    expect(resolvePlaybackHotkey(" ", true)).toBeNull();
    expect(resolvePlaybackHotkey("j", true)).toBeNull();
    expect(resolvePlaybackHotkey("j", false, true)).toBeNull();
  });
});
