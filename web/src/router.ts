import { errorView, renderLayout } from "./ui";
import {
  clearUnsavedChangesGuard,
  confirmDiscardChanges
} from "./navigation_guard";
import { accountView, invitationView, loginView, passwordView } from "./views/account";
import { adminPastes, adminView } from "./views/admin";
import { guideView } from "./views/guide";
import { home, pasteForm, pasteList, pasteView } from "./views/pastes";

type ScrollPosition = { scroll_x: number; scroll_y: number };

let currentScroll: ScrollPosition = { scroll_x: 0, scroll_y: 0 };
let scrollRecordingPaused = false;
let scrollFrame: number | undefined;

function scrollPosition(state: unknown = history.state): ScrollPosition {
  const candidate = state as Partial<ScrollPosition> | null;
  return {
    scroll_x: typeof candidate?.scroll_x === "number" ? candidate.scroll_x : 0,
    scroll_y: typeof candidate?.scroll_y === "number" ? candidate.scroll_y : 0
  };
}

function saveScrollPosition(position: ScrollPosition = {
  scroll_x: window.scrollX,
  scroll_y: window.scrollY
}): void {
  currentScroll = position;
  history.replaceState({ ...(history.state ?? {}), ...position }, "");
}

function nextPaint(): Promise<void> {
  return new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
}

async function renderAt(position: ScrollPosition): Promise<void> {
  scrollRecordingPaused = true;
  await route();
  await nextPaint();
  window.scrollTo(position.scroll_x, position.scroll_y);
  saveScrollPosition(position);
  scrollRecordingPaused = false;
}

export function initializeScrollRestoration(): void {
  history.scrollRestoration = "manual";
  currentScroll = {
    scroll_x: window.scrollX,
    scroll_y: window.scrollY
  };
  saveScrollPosition(currentScroll);
  window.addEventListener("scroll", () => {
    if (scrollRecordingPaused || scrollFrame !== undefined) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = undefined;
      if (!scrollRecordingPaused) saveScrollPosition();
    });
  }, { passive: true });
}

export function initialRoute(): Promise<void> {
  return renderAt(scrollPosition());
}

export function navigate(path: string): void {
  void (async () => {
    if (!(await confirmDiscardChanges())) return;
    clearUnsavedChangesGuard();
    saveScrollPosition();
    const top = { scroll_x: 0, scroll_y: 0 };
    history.pushState(top, "", path);
    await renderAt(top);
  })();
}

export function handlePopState(previousPath: string, state: unknown): void {
  scrollRecordingPaused = true;
  const previousScroll = currentScroll;
  void (async () => {
    if (!(await confirmDiscardChanges())) {
      history.pushState(previousScroll, "", previousPath);
      window.scrollTo(previousScroll.scroll_x, previousScroll.scroll_y);
      scrollRecordingPaused = false;
      return;
    }
    clearUnsavedChangesGuard();
    await renderAt(scrollPosition(state));
  })();
}

export async function route(): Promise<void> {
  const path = location.pathname;
  document.body.dataset.routePath = `${path}${location.search}`;
  try {
    if (path === "/") return await home();
    if (path === "/explore") return await pasteList(false);
    if (path === "/login") return loginView();
    if (path === "/pastes/new") return pasteForm();
    if (path === "/pastes") return await pasteList(true);
    if (path === "/account") return await accountView();
    if (path === "/account/password") return passwordView();
    if (path === "/admin") return adminView();
    if (path === "/admin/pastes") return await adminPastes();
    if (path === "/guide") return guideView();
    if (path.startsWith("/invitations/")) return invitationView(path.slice(13));
    const edit = path.match(/^\/pastes\/([^/]+)\/edit$/);
    if (edit?.[1]) return await pasteForm(edit[1]);
    const view = path.match(/^\/pastes\/([^/]+)$/);
    if (view?.[1]) return await pasteView(view[1]);
    renderLayout(
      `<section class="empty"><h1>Page not found</h1><p>The requested page does not exist.</p></section>`
    );
  } catch (error) {
    errorView(error);
  }
}
