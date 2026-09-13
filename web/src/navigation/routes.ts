import manifest from "./routes.json";

export type Route =
  | { name: "home" }
  | { name: "explore" }
  | { name: "login" }
  | { name: "new-paste" }
  | { name: "my-pastes" }
  | { name: "paste"; pasteId: string }
  | { name: "edit-paste"; pasteId: string }
  | { name: "account" }
  | { name: "password" }
  | { name: "admin" }
  | { name: "admin-pastes" }
  | { name: "admin-users" }
  | { name: "admin-user"; userId: number }
  | { name: "admin-invitations" }
  | { name: "admin-api-keys" }
  | { name: "admin-settings" }
  | { name: "admin-audit" }
  | { name: "help" }
  | { name: "password-reset"; token: string }
  | { name: "invitation"; token: string }
  | { name: "not-found" };

export type RouteName = Route["name"];
export type RouteAccess = "public" | "authenticated" | "admin" | "owner";
export type RouteLocation = {
  route: Route;
  path: string;
  query: URLSearchParams;
  hash: string;
};

type RouteDefinition = {
  name: Exclude<RouteName, "not-found">;
  path: string;
  title: string;
  access: RouteAccess;
};

export const routeDefinitions = manifest as RouteDefinition[];
const definitionsByName = new Map<RouteName, RouteDefinition>(
  routeDefinitions.map((definition) => [definition.name, definition])
);

function matchDefinition(definition: RouteDefinition, path: string): Route | null {
  const expected = definition.path.split("/");
  const actual = path.split("/");
  if (expected.length !== actual.length) return null;
  const parameters: Record<string, string | number> = {};
  for (let index = 0; index < expected.length; index += 1) {
    const segment = expected[index]!;
    const value = actual[index]!;
    if (!segment.startsWith(":")) {
      if (segment !== value) return null;
      continue;
    }
    if (!value) return null;
    const [name, kind] = segment.slice(1).split(":");
    if (!name) return null;
    if (kind === "int") {
      if (!/^\d+$/.test(value)) return null;
      parameters[name] = Number(value);
    } else {
      parameters[name] = value;
    }
  }
  return { name: definition.name, ...parameters } as Route;
}

export function parseRoute(path: string): Route {
  for (const definition of routeDefinitions) {
    const route = matchDefinition(definition, path);
    if (route) return route;
  }
  return { name: "not-found" };
}

export function parseLocation(path: string, search = "", hash = ""): RouteLocation {
  return { route: parseRoute(path), path, query: new URLSearchParams(search), hash };
}

export function routeTitle(route: Route): string {
  return definitionsByName.get(route.name)?.title ?? "Page not found";
}

export function routeAccess(route: Route): RouteAccess {
  return definitionsByName.get(route.name)?.access ?? "public";
}

export const spaRouteSamples = routeDefinitions.map((definition) =>
  definition.path
    .replace(":userId:int", "42")
    .replace(":pasteId", "example")
    .replace(":token", "token")
);
