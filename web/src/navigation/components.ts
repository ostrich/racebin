import type { Component } from "svelte";
import { parseRoute, type Route, type RouteName } from "./routes";

type PageModule = { default: Component<any> };
type PageLoader = () => Promise<PageModule>;
export type HomeRouteVariant = "public" | "login" | "authenticated";

const homeLoaders: Record<HomeRouteVariant, PageLoader> = {
  public: () => import("../pages/HomePage.svelte"),
  login: () => import("../pages/LoginPage.svelte"),
  authenticated: () => import("../pages/PasteFormPage.svelte")
};

const loaders: Record<Exclude<RouteName, "home">, PageLoader> = {
  explore: () => import("../pages/PasteListPage.svelte"),
  login: () => import("../pages/LoginPage.svelte"),
  "new-paste": () => import("../pages/PasteFormPage.svelte"),
  "my-pastes": () => import("../pages/PasteListPage.svelte"),
  paste: () => import("../pages/PasteViewPage.svelte"),
  "edit-paste": () => import("../pages/PasteFormPage.svelte"),
  account: () => import("../pages/AccountPage.svelte"),
  password: () => import("../pages/PasswordPage.svelte"),
  admin: () => import("../pages/admin/DashboardPage.svelte"),
  "admin-pastes": () => import("../pages/admin/PastesPage.svelte"),
  "admin-users": () => import("../pages/admin/UsersPage.svelte"),
  "admin-user": () => import("../pages/admin/UserPage.svelte"),
  "admin-invitations": () => import("../pages/admin/InvitationsPage.svelte"),
  "admin-api-keys": () => import("../pages/admin/ApiKeysPage.svelte"),
  "admin-settings": () => import("../pages/admin/SettingsPage.svelte"),
  "admin-audit": () => import("../pages/admin/AuditPage.svelte"),
  help: () => import("../pages/HelpPage.svelte"),
  "password-reset": () => import("../pages/PasswordResetPage.svelte"),
  invitation: () => import("../pages/InvitationPage.svelte"),
  "not-found": () => import("../pages/NotFoundPage.svelte")
};

const loaded = new Map<string, Promise<PageModule>>();

function load(key: string, loader: PageLoader): Promise<PageModule> {
  let request = loaded.get(key);
  if (!request) {
    request = loader().catch((error) => {
      loaded.delete(key);
      throw error;
    });
    loaded.set(key, request);
  }
  return request;
}

export function loadRouteComponent(
  route: Route,
  homeVariant: HomeRouteVariant
): Promise<PageModule> {
  return route.name === "home"
    ? load(`home:${homeVariant}`, homeLoaders[homeVariant])
    : load(route.name, loaders[route.name]);
}

type NetworkInformation = { saveData?: boolean; effectiveType?: string };

export function routePrefetchAllowed(connection?: NetworkInformation): boolean {
  return !connection?.saveData && !["slow-2g", "2g"].includes(connection?.effectiveType ?? "");
}

export function prefetchRoute(href: string): void {
  const connection = (navigator as Navigator & { connection?: NetworkInformation }).connection;
  if (!routePrefetchAllowed(connection)) return;
  const url = new URL(href, location.href);
  if (url.origin !== location.origin) return;
  const route = parseRoute(url.pathname);
  if (route.name === "home" || route.name === "not-found") return;
  void load(route.name, loaders[route.name]);
}

export function routeComponentKey(
  route: Route,
  query: URLSearchParams,
  homeVariant: HomeRouteVariant = "public"
): string {
  const parameter =
    "pasteId" in route
      ? route.pasteId
      : "userId" in route
        ? route.userId
        : "token" in route
          ? route.token
          : "";
  const queryIdentity = route.name === "new-paste" ? query.toString() : "";
  const stateIdentity = route.name === "home" ? homeVariant : "";
  return `${route.name}:${parameter}:${queryIdentity}:${stateIdentity}`;
}

export function routeProps(route: Route, query: URLSearchParams): Record<string, unknown> {
  switch (route.name) {
    case "explore":
      return { mine: false, query };
    case "my-pastes":
      return { mine: true, query };
    case "paste":
      return { pasteId: route.pasteId };
    case "edit-paste":
      return { pasteId: route.pasteId };
    case "account":
    case "admin-pastes":
    case "admin-users":
    case "admin-invitations":
    case "admin-api-keys":
    case "admin-audit":
      return { query };
    case "admin-user":
      return { userId: route.userId };
    case "password-reset":
    case "invitation":
      return { token: route.token };
    default:
      return {};
  }
}
