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

  it("broadcasts queue changes made from a WebView", () => {
    expect(commandsSource).toContain("fn emit_playback_updated");

    for (const command of [
      "pub fn play_track(",
      "pub fn queue_track_next(",
      "pub fn add_to_queue(",
      "pub fn move_queue_item(",
    ]) {
      const start = commandsSource.lastIndexOf(command);
      const end = commandsSource.indexOf(
        "\n#[tauri::command]",
        start + command.length,
      );
      const commandSource = commandsSource.slice(start, end);

      expect(commandSource).toContain(
        "emit_playback_updated(&app, &snapshot);",
      );
    }
  });
});
