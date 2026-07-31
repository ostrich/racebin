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

  it("parses help, recovery, and user administration paths", () => {
    expect(parseRoute("/help")).toEqual({ name: "help" });
    expect(parseRoute("/admin/users")).toEqual({ name: "admin-users" });
    expect(parseRoute("/admin/users/42")).toEqual({ name: "admin-user", userId: 42 });
    expect(parseRoute("/password-reset/example")).toEqual({ name: "password-reset", token: "example" });
    expect(parseRoute("/admin/users/not-a-number")).toEqual({ name: "not-found" });
  });
});
