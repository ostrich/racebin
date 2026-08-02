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
  | { name: "help" }
  | { name: "password-reset"; token: string }
  | { name: "invitation"; token: string }
  | { name: "not-found" };

export type RouteLocation = {
  route: Route;
  path: string;
  query: URLSearchParams;
};

export function parseRoute(path: string): Route {
  if (path === "/") return { name: "home" };
  if (path === "/explore") return { name: "explore" };
  if (path === "/login") return { name: "login" };
  if (path === "/pastes/new") return { name: "new-paste" };
  if (path === "/pastes") return { name: "my-pastes" };
  if (path === "/account") return { name: "account" };
  if (path === "/account/password") return { name: "password" };
  if (path === "/admin") return { name: "admin" };
  if (path === "/admin/pastes") return { name: "admin-pastes" };
  if (path === "/admin/users") return { name: "admin-users" };
  if (path === "/help") return { name: "help" };
  const adminUser = path.match(/^\/admin\/users\/(\d+)$/);
  if (adminUser?.[1]) return { name: "admin-user", userId: Number(adminUser[1]) };
  const reset = path.match(/^\/password-reset\/([^/]+)$/);
  if (reset?.[1]) return { name: "password-reset", token: reset[1] };
  const invitation = path.match(/^\/invitations\/([^/]+)$/);
  if (invitation?.[1]) return { name: "invitation", token: invitation[1] };
  const edit = path.match(/^\/pastes\/([^/]+)\/edit$/);
  if (edit?.[1]) return { name: "edit-paste", pasteId: edit[1] };
  const paste = path.match(/^\/pastes\/([^/]+)$/);
  if (paste?.[1]) return { name: "paste", pasteId: paste[1] };
  return { name: "not-found" };
}

export function parseLocation(path: string, search = ""): RouteLocation {
  return { route: parseRoute(path), path, query: new URLSearchParams(search) };
}

export function routeTitle(route: Route): string {
  switch (route.name) {
    case "home": return "Home";
    case "explore": return "Explore";
    case "login": return "Log in";
    case "new-paste": return "New paste";
    case "my-pastes": return "My pastes";
    case "paste": return "Paste";
    case "edit-paste": return "Edit paste";
    case "account": return "Account";
    case "password": return "Change password";
    case "admin": return "Administration";
    case "admin-pastes": return "Manage pastes";
    case "admin-users": return "Manage users";
    case "admin-user": return "Manage user";
    case "help": return "Help";
    case "password-reset": return "Reset password";
    case "invitation": return "Invitation";
    case "not-found": return "Page not found";
  }
}
