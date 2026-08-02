import { beforeEach, describe, expect, it } from "vitest";
import { historyIndex, savedScroll, stateWithNavigation } from "./scroll";

describe("navigation history state", () => {
  beforeEach(() => history.replaceState({ application: "preserved" }, "", "/"));

  it("preserves unrelated state while recording entry identity and scroll", () => {
    const state = stateWithNavigation(4, { x: 12, y: 300 });
    expect(state.application).toBe("preserved");
    expect(historyIndex(state)).toBe(4);
    expect(savedScroll(state)).toEqual({ x: 12, y: 300 });
  });

  it("safely defaults malformed or absent scroll state", () => {
    expect(historyIndex({ racebin: { index: "bad" } })).toBeUndefined();
    expect(savedScroll(null)).toEqual({ x: 0, y: 0 });
  });
});
