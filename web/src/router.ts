import { tick } from "svelte";
import { get, writable } from "svelte/store";

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
  | { name: "guide" }
  | { name: "invitation"; token: string }
  | { name: "not-found" };

export type LocationState = {
  route: Route;
  path: string;
  query: URLSearchParams;
};

type ScrollPosition = { scroll_x: number; scroll_y: number };
type DiscardPrompt = () => Promise<boolean>;
type RouteReadiness = {
  pending: number;
  sealed: boolean;
  promise: Promise<void>;
  resolve: () => void;
};

let dirtyCheck: (() => boolean) | undefined;
let discardPrompt: DiscardPrompt = async () => window.confirm("Discard unsaved changes?");
let currentPath = "/";
let currentScroll: ScrollPosition = { scroll_x: 0, scroll_y: 0 };
let scrollPaused = false;
let scrollFrame: number | undefined;
let routeReadiness = createRouteReadiness();

export const locationState = writable<LocationState>(
  parseLocation(location.pathname, location.search)
);

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
  if (path === "/guide") return { name: "guide" };
  const invitation = path.match(/^\/invitations\/([^/]+)$/);
  if (invitation?.[1]) return { name: "invitation", token: invitation[1] };
  const edit = path.match(/^\/pastes\/([^/]+)\/edit$/);
  if (edit?.[1]) return { name: "edit-paste", pasteId: edit[1] };
  const paste = path.match(/^\/pastes\/([^/]+)$/);
  if (paste?.[1]) return { name: "paste", pasteId: paste[1] };
  return { name: "not-found" };
}

function parseLocation(path: string, search: string): LocationState {
  return { route: parseRoute(path), path, query: new URLSearchParams(search) };
}

function scrollPosition(state: unknown = history.state): ScrollPosition {
  const candidate = state as Partial<ScrollPosition> | null;
  return {
    scroll_x: typeof candidate?.scroll_x === "number" ? candidate.scroll_x : 0,
    scroll_y: typeof candidate?.scroll_y === "number" ? candidate.scroll_y : 0
  };
}

function saveScroll(position: ScrollPosition = {
  scroll_x: window.scrollX,
  scroll_y: window.scrollY
}): void {
  currentScroll = position;
  history.replaceState({ ...(history.state ?? {}), ...position }, "");
}

function createRouteReadiness(): RouteReadiness {
  let resolve = () => {};
  const promise = new Promise<void>(ready => {
    resolve = ready;
  });
  return { pending: 0, sealed: false, promise, resolve };
}

function beginRouteReadiness(): RouteReadiness {
  routeReadiness.resolve();
  routeReadiness = createRouteReadiness();
  return routeReadiness;
}

function settleRouteReadiness(readiness: RouteReadiness): void {
  if (readiness.sealed && readiness.pending === 0) readiness.resolve();
}

/**
 * Defers scroll restoration until this route's initial asynchronous rendering
 * is complete. The returned callback is idempotent and scoped to the current
 * navigation, so a stale request cannot release a newer route.
 */
export function deferRouteReady(): () => void {
  const readiness = routeReadiness;
  readiness.pending += 1;
  let complete = false;
  return () => {
    if (complete) return;
    complete = true;
    readiness.pending -= 1;
    settleRouteReadiness(readiness);
  };
}

async function renderAt(position: ScrollPosition): Promise<void> {
  const readiness = beginRouteReadiness();
  scrollPaused = true;
  currentPath = `${location.pathname}${location.search}`;
  locationState.set(parseLocation(location.pathname, location.search));
  await tick();
  readiness.sealed = true;
  settleRouteReadiness(readiness);
  await readiness.promise;
  if (readiness !== routeReadiness) return;
  await tick();
  await new Promise<void>(resolve =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  );
  if (readiness !== routeReadiness) return;
  window.scrollTo(position.scroll_x, position.scroll_y);
  saveScroll();
  scrollPaused = false;
}

export function setDiscardPrompt(prompt: DiscardPrompt): void {
  discardPrompt = prompt;
}

export function guardUnsavedChanges(check?: () => boolean): void {
  dirtyCheck = check;
}

export function hasUnsavedChanges(): boolean {
  return dirtyCheck?.() ?? false;
}

export async function confirmDiscardChanges(): Promise<boolean> {
  return !hasUnsavedChanges() || discardPrompt();
}

export async function navigate(path: string, options: { replace?: boolean } = {}): Promise<boolean> {
  if (!(await confirmDiscardChanges())) return false;
  dirtyCheck = undefined;
  saveScroll();
  const top = { scroll_x: 0, scroll_y: 0 };
  if (options.replace) history.replaceState(top, "", path);
  else history.pushState(top, "", path);
  await renderAt(top);
  return true;
}

export function initializeRouter(): () => void {
  history.scrollRestoration = "manual";
  currentPath = `${location.pathname}${location.search}`;
  currentScroll = { scroll_x: window.scrollX, scroll_y: window.scrollY };
  saveScroll(currentScroll);

  const onScroll = () => {
    if (scrollPaused || scrollFrame !== undefined) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = undefined;
      if (!scrollPaused) saveScroll();
    });
  };
  const onPopState = (event: PopStateEvent) => {
    scrollPaused = true;
    const previousPath = currentPath;
    const previousScroll = currentScroll;
    void (async () => {
      if (!(await confirmDiscardChanges())) {
        history.pushState(previousScroll, "", previousPath);
        window.scrollTo(previousScroll.scroll_x, previousScroll.scroll_y);
        scrollPaused = false;
        return;
      }
      dirtyCheck = undefined;
      await renderAt(scrollPosition(event.state));
    })();
  };
  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("popstate", onPopState);
  void renderAt(scrollPosition());
  return () => {
    window.removeEventListener("scroll", onScroll);
    window.removeEventListener("popstate", onPopState);
  };
}

export function currentLocation(): LocationState {
  return get(locationState);
}
