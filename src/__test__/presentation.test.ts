import { describe, expect, it } from "vitest";

import { playerDimensions } from "../presentation";

describe("playerDimensions", () => {
  it("uses a smaller compact footprint when the queue is collapsed", () => {
    expect(playerDimensions(false)).toEqual({
      width: 480,
      height: 144,
    });
  });

  it("allocates space for the queue when the mini player is expanded", () => {
    expect(playerDimensions(true)).toEqual({
      width: 480,
      height: 390,
    });
  });
});
