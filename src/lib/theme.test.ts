import { describe, expect, it } from "vitest";

import { readTheme } from "./theme";

describe("readTheme", () => {
  it("uses the saved supported theme", () => {
    expect(readTheme("plum")).toBe("plum");
  });

  it("falls back to midnight for an unknown value", () => {
    expect(readTheme("violet")).toBe("midnight");
  });
});
