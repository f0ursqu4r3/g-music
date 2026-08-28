import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

interface CapabilityDocument {
  permissions: string[];
  windows: string[];
}

function readCapability(path: string): CapabilityDocument {
  return JSON.parse(readFileSync(path, "utf8")) as CapabilityDocument;
}

describe("desktop window capabilities", () => {
  it("grants shared playback commands to every native window", () => {
    expect(
      readCapability("src-tauri/capabilities/default.json").windows,
    ).toEqual(["main", "artwork", "queue", "mini-player"]);
  });

  it("allows the player to resize its native window", () => {
    expect(
      readCapability("src-tauri/capabilities/mini-player.json").permissions,
    ).toEqual(
      expect.arrayContaining([
        "core:window:allow-close",
        "core:window:allow-set-resizable",
        "core:window:allow-set-size",
      ]),
    );
  });

  it("allows only draggable surfaces to start native window dragging", () => {
    const draggingCapability = readCapability(
      "src-tauri/capabilities/window-dragging.json",
    );

    expect(draggingCapability.windows).toEqual(["main", "mini-player"]);
    expect(draggingCapability.permissions).toContain(
      "core:window:allow-start-dragging",
    );
  });

  it("scopes the mutation permissions to the compact mini player", () => {
    expect(
      readCapability("src-tauri/capabilities/mini-player.json").windows,
    ).toEqual(expect.arrayContaining(["mini-player"]));
  });
});
