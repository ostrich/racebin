import { describe, expect, it } from "vitest";
import { parseLocation, parseRoute, routeTitle } from "./routes";

describe("routes", () => {
  it("parses static and parameterized routes without accepting trailing paths", () => {
    expect(parseRoute("/pastes/sample-paste")).toEqual({ name: "paste", pasteId: "sample-paste" });
    expect(parseRoute("/pastes/sample-paste/edit")).toEqual({ name: "edit-paste", pasteId: "sample-paste" });
    expect(parseRoute("/admin/users/42")).toEqual({ name: "admin-user", userId: 42 });
    expect(parseRoute("/pastes/example/extra")).toEqual({ name: "not-found" });
    expect(parseRoute("/admin/users/not-a-number")).toEqual({ name: "not-found" });
  });

  it("keeps URL query state separate from route matching", () => {
    const location = parseLocation("/pastes", "?folder_id=7&sort=title");
    expect(location.route).toEqual({ name: "my-pastes" });
    expect(location.query.get("folder_id")).toBe("7");
    expect(location.query.get("sort")).toBe("title");
  });

  it("provides a title for every route", () => {
    expect(routeTitle({ name: "new-paste" })).toBe("New paste");
    expect(routeTitle({ name: "not-found" })).toBe("Page not found");
  });
});
