import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const componentNames = [
  "ArtworkWindow",
  "LibraryWindow",
  "MiniPlayer",
  "MiniWindow",
  "QueueWindow",
] as const;

const globalComponentSelectors = [
  ".artwork-window",
  ".library-window",
  ".mini-player-shell",
  ".mini-window-shell",
  ".player-frame",
  ".queue-drawer",
  ".queue-window",
];

describe("component style locality", () => {
  it("keeps component selectors out of the global stylesheet", () => {
    const globalStyles = readFileSync(
      join(process.cwd(), "src/styles.css"),
      "utf8",
    );

    for (const selector of globalComponentSelectors) {
      expect(globalStyles).not.toContain(selector);
    }
  });

  it("uses native-style text selection while preserving editable text", () => {
    const globalStyles = readFileSync(
      join(process.cwd(), "src/styles.css"),
      "utf8",
    );

    expect(globalStyles).toMatch(/body\s*{[^}]*user-select:\s*none/s);
    expect(globalStyles).toMatch(
      /input,\s*textarea,\s*\[contenteditable=["']true["']\]\s*{[^}]*user-select:\s*text/s,
    );
  });

  it("keeps non-utility presentation next to each visual component", () => {
    for (const componentName of componentNames) {
      const source = readFileSync(
        join(process.cwd(), `src/components/${componentName}.vue`),
        "utf8",
      );

      expect(source).toContain("<style scoped>");
    }
  });
});
