import { describe, expect, it } from "vitest";
import { formatTimecode, seconds, toSeconds } from "./time";

describe("rational time", () => {
  it("round-trips common edit times", () => {
    expect(toSeconds(seconds(1.5))).toBe(1.5);
    expect(seconds(1 / 30)).toEqual({ value: 20, timescale: 600 });
  });

  it("formats a frame-aware timecode", () => {
    expect(formatTimecode(seconds(65.5), 30)).toBe("00:01:05:15");
  });

  it("rejects invalid timescales", () => {
    expect(() => toSeconds({ value: 1, timescale: 0 })).toThrow("timescale must be positive");
  });
});
