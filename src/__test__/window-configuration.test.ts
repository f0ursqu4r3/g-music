import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

interface MainWindowConfiguration {
  hiddenTitle?: boolean;
  theme?: string;
  titleBarStyle?: string;
  transparent?: boolean;
  trafficLightPosition?: { x: number; y: number };
}

interface TauriConfiguration {
  app: { windows: MainWindowConfiguration[] };
}

function mainWindow(): MainWindowConfiguration {
  const configuration = JSON.parse(
    readFileSync("src-tauri/tauri.conf.json", "utf8"),
  ) as TauriConfiguration;

  return configuration.app.windows[0];
}

describe("main window chrome", () => {
  it("uses a dark overlay title bar for a seamless glass surface", () => {
    expect(mainWindow()).toMatchObject({
      hiddenTitle: true,
      theme: "Dark",
      titleBarStyle: "Overlay",
      transparent: true,
      trafficLightPosition: expect.objectContaining({ x: 16, y: 18 }),
    });
  });
});
