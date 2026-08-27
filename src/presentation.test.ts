import { describe, expect, it } from "vitest";

import { playerDimensions } from "./presentation";

describe("playerDimensions", () => {
  it("allocates space for the queue when the mini player is expanded", () => {
    expect(playerDimensions(true)).toEqual({
      width: 520,
      height: 420,
    });
  });
});
