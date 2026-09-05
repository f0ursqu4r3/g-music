import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const windowsSource = readFileSync("src-tauri/src/windows.rs", "utf8");
const commandsSource = readFileSync("src-tauri/src/commands.rs", "utf8");

describe("native menu and menu hotkeys", () => {
  it("declares Playback and keyboard-shortcuts menu sections", () => {
    expect(windowsSource).toContain('"Playback"');
    expect(windowsSource).toContain('"Keyboard Shortcuts…"');
  });

  it("registers standard accelerators for import, transport, and windows", () => {
    for (const accelerator of [
      "CmdOrCtrl+I",
      "Space",
      "CmdOrCtrl+Left",
      "CmdOrCtrl+Right",
      "CmdOrCtrl+1",
      "CmdOrCtrl+2",
      "CmdOrCtrl+3",
      "CmdOrCtrl+4",
      "CmdOrCtrl+/",
    ]) {
      expect(windowsSource).toContain(`accelerator("${accelerator}")`);
    }
  });

  it("routes native playback actions through application state and broadcasts snapshots", () => {
    expect(commandsSource).toContain('"playback.toggle"');
    expect(commandsSource).toContain('"playback.previous"');
    expect(commandsSource).toContain('"playback.next"');
    expect(commandsSource).toContain('"playback-updated"');
  });

  // Queue event reception is exercised through real mounted App and Queue
  // components in App.hardening.test.ts. Backend tests own event emission.
});
