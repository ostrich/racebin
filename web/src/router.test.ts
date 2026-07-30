import { describe, expect, it } from "vitest";
import { parseRoute } from "./router";

describe("parseRoute", () => {
  it("parses every parameterized paste route", () => {
    expect(parseRoute("/pastes/sample-paste")).toEqual({
      name: "paste",
      pasteId: "sample-paste"
    });
    expect(parseRoute("/pastes/sample-paste/edit")).toEqual({
      name: "edit-paste",
      pasteId: "sample-paste"
    });
  });

  it("rejects nested and obsolete routes", () => {
    expect(parseRoute("/new")).toEqual({ name: "not-found" });
    expect(parseRoute("/pastes/example/extra")).toEqual({ name: "not-found" });
    expect(parseRoute("/invitations/token/extra")).toEqual({ name: "not-found" });
  });
});
