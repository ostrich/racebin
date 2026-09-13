import { describe, expect, it } from "vitest";
import { routeComponentKey } from "./components";

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
});
