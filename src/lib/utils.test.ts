import { describe, expect, it } from "vitest";

import { isProbablyUrl } from "./utils";

describe("isProbablyUrl", () => {
  it("accepts web links with or without an explicit scheme", () => {
    expect(isProbablyUrl("https://example.com/video")).toBe(true);
    expect(isProbablyUrl("example.com/video")).toBe(true);
  });

  it("rejects non-web schemes and incomplete hosts", () => {
    expect(isProbablyUrl("javascript://example.com/payload")).toBe(false);
    expect(isProbablyUrl("file://example.com/video.mp4")).toBe(false);
    expect(isProbablyUrl("localhost/video")).toBe(false);
  });
});
