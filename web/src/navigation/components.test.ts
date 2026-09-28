import { describe, expect, it } from "vitest";
import { routeComponentKey, routePrefetchAllowed } from "./components";

describe("route component identity", () => {
  it("remounts new-paste forms for meaningful query changes", () => {
    expect(routeComponentKey({ name: "new-paste" }, new URLSearchParams("folder_id=7"))).not.toBe(
      routeComponentKey({ name: "new-paste" }, new URLSearchParams())
    );
  });

  it("keeps list pages mounted while their query changes", () => {
    expect(routeComponentKey({ name: "my-pastes" }, new URLSearchParams("folder_id=7"))).toBe(
      routeComponentKey({ name: "my-pastes" }, new URLSearchParams())
    );
  });

  it("remounts the state-dependent home route when its variant changes", () => {
    expect(routeComponentKey({ name: "home" }, new URLSearchParams(), "authenticated")).not.toBe(
      routeComponentKey({ name: "home" }, new URLSearchParams(), "login")
    );
  });
});

describe("route prefetch policy", () => {
  it("avoids speculative downloads when data saving is requested", () => {
    expect(routePrefetchAllowed({ saveData: true, effectiveType: "4g" })).toBe(false);
  });

  it("avoids speculative downloads on very slow connections", () => {
    expect(routePrefetchAllowed({ effectiveType: "2g" })).toBe(false);
    expect(routePrefetchAllowed({ effectiveType: "slow-2g" })).toBe(false);
  });

  it("prefetches on ordinary and moderately constrained connections", () => {
    expect(routePrefetchAllowed()).toBe(true);
    expect(routePrefetchAllowed({ effectiveType: "3g" })).toBe(true);
  });
});
