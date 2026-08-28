import { describe, expect, it } from "vitest";

import { formatDuration } from "../time";

describe("formatDuration", () => {
  it("formats minutes and zero-padded seconds", () => {
    expect(formatDuration(63_000)).toBe("1:03");
  });

  it("never returns a negative time", () => {
    expect(formatDuration(-1)).toBe("0:00");
  });
});
