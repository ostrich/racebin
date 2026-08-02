import { tick } from "svelte";
import { get, writable } from "svelte/store";
import {
  clearUnsavedChangesGuard,
  confirmDiscardChanges,
  startUnloadGuard
} from "./guards";
import { parseLocation, routeTitle, type RouteLocation } from "./routes";
import {
  historyIndex,
  replaceSavedScroll,
  restoreScroll,
  savedScroll,
  stateWithNavigation,
  type ScrollPosition
} from "./scroll";

type NavigationKind = "initial" | "push" | "replace" | "pop";
type AccessPolicy = (location: RouteLocation) => string | null | Promise<string | null>;
type NavigationOptions = { replace?: boolean };
type RuntimeOptions = { accessPolicy?: AccessPolicy; siteName?: () => string };

type NavigationTransaction = {
  id: number;
  pending: number;
  sealed: boolean;
  ready: Promise<void>;
  resolve: () => void;
};

export const locationState = writable<RouteLocation>(
  parseLocation(location.pathname, location.search, location.hash)
);
export const navigationReady = writable(false);

let transactionSequence = 0;
let activeTransaction: NavigationTransaction | undefined;
let currentIndex = 0;
let currentPath = `${location.pathname}${location.search}${location.hash}`;
let scrollPaused = false;
let scrollFrame: number | undefined;
let reversingPop = false;
let stopRuntime: (() => void) | undefined;
let options: RuntimeOptions = {};

function transaction(): NavigationTransaction {
  let resolve = () => {};
  const ready = new Promise<void>(complete => { resolve = complete; });
  return { id: ++transactionSequence, pending: 0, sealed: false, ready, resolve };
}

function settle(candidate: NavigationTransaction): void {
  if (candidate.sealed && candidate.pending === 0) candidate.resolve();
}

/**
 * Holds completion of the navigation that mounted the calling page. The
 * release function is idempotent and cannot affect a later navigation.
 */
export function holdNavigation(): () => void {
  const owner = activeTransaction;
  if (!owner || owner.sealed) return () => {};
  owner.pending += 1;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    owner.pending -= 1;
    settle(owner);
  };
}

async function allowedLocation(path: string): Promise<{ path: string; location: RouteLocation }> {
  let candidate = new URL(path, location.href);
  for (let redirects = 0; redirects < 8; redirects += 1) {
    if (candidate.origin !== location.origin) throw new Error("Navigation must remain on this site");
    const parsed = parseLocation(candidate.pathname, candidate.search, candidate.hash);
    const redirect = await options.accessPolicy?.(parsed);
    if (!redirect) {
      return { path: `${candidate.pathname}${candidate.search}${candidate.hash}`, location: parsed };
    }
    candidate = new URL(redirect, candidate);
  }
  throw new Error("Navigation access policy produced a redirect loop");
}

function updateDocument(location: RouteLocation): void {
  const siteName = options.siteName?.() || "Racebin";
  document.title = `${routeTitle(location.route)} · ${siteName}`;
}

function focusRoute(): void {
  const heading = document.querySelector<HTMLElement>("main h1");
  const target = heading ?? document.querySelector<HTMLElement>("main");
  if (!target) return;
  const hadTabIndex = target.hasAttribute("tabindex");
  if (!hadTabIndex) target.setAttribute("tabindex", "-1");
  target.focus({ preventScroll: true });
  if (!hadTabIndex) target.addEventListener("blur", () => target.removeAttribute("tabindex"), { once: true });
}

function revealFragment(hash: string): boolean {
  if (!hash) return false;
  let id: string;
  try {
    id = decodeURIComponent(hash.slice(1));
  } catch {
    return false;
  }
  const target = document.getElementById(id);
  if (!target) return false;
  target.scrollIntoView({ block: "start" });
  const focusTarget = target.matches("h1, h2, h3, h4, h5, h6")
    ? target
    : target.querySelector<HTMLElement>("h1, h2, h3, h4, h5, h6");
  if (focusTarget) {
    const hadTabIndex = focusTarget.hasAttribute("tabindex");
    if (!hadTabIndex) focusTarget.setAttribute("tabindex", "-1");
    focusTarget.focus({ preventScroll: true });
    if (!hadTabIndex) {
      focusTarget.addEventListener("blur", () => focusTarget.removeAttribute("tabindex"), { once: true });
    }
  }
  return true;
}

async function commit(
  requestedPath: string,
  kind: NavigationKind,
  position: ScrollPosition,
  state?: unknown
): Promise<boolean> {
  const owner = transaction();
  activeTransaction?.resolve();
  activeTransaction = owner;
  scrollPaused = true;

  const allowed = await allowedLocation(requestedPath);
  if (activeTransaction !== owner) return false;

  if (kind === "initial") {
    currentIndex = historyIndex(state) ?? 0;
    history.replaceState(stateWithNavigation(currentIndex, position, state), "", allowed.path);
  } else if (kind === "push") {
    currentIndex += 1;
    history.pushState(stateWithNavigation(currentIndex, position), "", allowed.path);
  } else if (kind === "replace") {
    history.replaceState(stateWithNavigation(currentIndex, position), "", allowed.path);
  } else if (allowed.path !== requestedPath) {
    history.replaceState(stateWithNavigation(currentIndex, position, state), "", allowed.path);
  }

  currentPath = allowed.path;
  navigationReady.set(true);
  locationState.set(allowed.location);
  updateDocument(allowed.location);
  await tick();
  owner.sealed = true;
  settle(owner);
  await owner.ready;
  if (activeTransaction !== owner) return false;
  await tick();
  await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
  if (activeTransaction !== owner) return false;
  const revealedFragment = revealFragment(allowed.location.hash);
  if (!revealedFragment) restoreScroll(position);
  replaceSavedScroll(currentIndex);
  scrollPaused = false;
  if (!revealedFragment && (kind === "push" || kind === "replace")) focusRoute();
  return true;
}

export async function navigate(path: string, navigation: NavigationOptions = {}): Promise<boolean> {
  if (!(await confirmDiscardChanges())) return false;
  clearUnsavedChangesGuard();
  replaceSavedScroll(currentIndex);
  const top = { x: 0, y: 0 };
  return commit(path, navigation.replace ? "replace" : "push", top);
}

export async function startNavigation(runtimeOptions: RuntimeOptions = {}): Promise<() => void> {
  stopRuntime?.();
  options = runtimeOptions;
  history.scrollRestoration = "manual";
  currentPath = `${location.pathname}${location.search}${location.hash}`;
  currentIndex = historyIndex() ?? 0;
  replaceSavedScroll(currentIndex);

  const stopUnloadGuard = startUnloadGuard();
  const onScroll = () => {
    if (scrollPaused || scrollFrame !== undefined) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = undefined;
      if (!scrollPaused) replaceSavedScroll(currentIndex);
    });
  };
  const onPopState = (event: PopStateEvent) => {
    scrollPaused = true;
    const targetIndex = historyIndex(event.state);
    if (reversingPop) {
      reversingPop = false;
      restoreScroll(savedScroll(event.state));
      scrollPaused = false;
      return;
    }
    void (async () => {
      if (!(await confirmDiscardChanges())) {
        if (targetIndex !== undefined) {
          reversingPop = true;
          history.go(currentIndex - targetIndex);
        } else {
          history.pushState(stateWithNavigation(currentIndex, savedScroll()), "", currentPath);
          scrollPaused = false;
        }
        return;
      }
      clearUnsavedChangesGuard();
      if (targetIndex !== undefined) currentIndex = targetIndex;
      await commit(
        `${location.pathname}${location.search}${location.hash}`,
        "pop",
        savedScroll(event.state),
        event.state
      );
    })();
  };
  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("popstate", onPopState);
  await commit(currentPath, "initial", savedScroll(), history.state);

  const stop = () => {
    window.removeEventListener("scroll", onScroll);
    window.removeEventListener("popstate", onPopState);
    stopUnloadGuard();
    if (scrollFrame !== undefined) cancelAnimationFrame(scrollFrame);
    scrollFrame = undefined;
    activeTransaction?.resolve();
    activeTransaction = undefined;
    clearUnsavedChangesGuard();
    stopRuntime = undefined;
  };
  stopRuntime = stop;
  return stop;
}

export function currentLocation(): RouteLocation {
  return get(locationState);
}
