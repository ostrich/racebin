import { describe, expect, it } from "vitest";
import {
  parseLocation,
  parseRoute,
  routeAccess,
  routeDefinitions,
  routeTitle,
  spaRouteSamples
} from "./routes";

describe("routes", () => {
  it("parses static and parameterized routes without accepting trailing paths", () => {
    expect(parseRoute("/pastes/sample-paste")).toEqual({ name: "paste", pasteId: "sample-paste" });
    expect(parseRoute("/pastes/sample-paste/edit")).toEqual({
      name: "edit-paste",
      pasteId: "sample-paste"
    });
    expect(parseRoute("/admin/users/42")).toEqual({ name: "admin-user", userId: 42 });
    expect(parseRoute("/admin/invitations")).toEqual({ name: "admin-invitations" });
    expect(parseRoute("/admin/api-keys")).toEqual({ name: "admin-api-keys" });
    expect(parseRoute("/admin/settings")).toEqual({ name: "admin-settings" });
    expect(parseRoute("/admin/audit")).toEqual({ name: "admin-audit" });
    expect(parseRoute("/pastes/example/extra")).toEqual({ name: "not-found" });
    expect(parseRoute("/admin/users/not-a-number")).toEqual({ name: "not-found" });
  });

  it("keeps URL query state separate from route matching", () => {
    const location = parseLocation("/pastes", "?folder_id=7&sort=title");
    expect(location.route).toEqual({ name: "my-pastes" });
    expect(location.query.get("folder_id")).toBe("7");
    expect(location.query.get("sort")).toBe("title");
    expect(location.hash).toBe("");
    expect(parseLocation("/help", "", "#scopes").hash).toBe("#scopes");
  });

  it("provides a title for every route", () => {
    expect(routeTitle({ name: "new-paste" })).toBe("New paste");
    expect(routeTitle({ name: "not-found" })).toBe("Page not found");
  });

  it("keeps every canonical route uniquely matchable and fully described", () => {
    expect(new Set(routeDefinitions.map((route) => route.name)).size).toBe(routeDefinitions.length);
    expect(new Set(routeDefinitions.map((route) => route.path)).size).toBe(routeDefinitions.length);
    for (const [index, sample] of spaRouteSamples.entries()) {
      const route = parseRoute(sample);
      expect(route.name, sample).toBe(routeDefinitions[index]!.name);
      expect(routeTitle(route), sample).toBe(routeDefinitions[index]!.title);
      expect(routeAccess(route), sample).toBe(routeDefinitions[index]!.access);
    }
  });
});
