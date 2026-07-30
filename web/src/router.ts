import { errorView, renderLayout } from "./ui";
import {
  clearUnsavedChangesGuard,
  confirmDiscardChanges
} from "./navigation_guard";
import { accountView, invitationView, loginView, passwordView } from "./views/account";
import { adminPastes, adminView } from "./views/admin";
import { guideView } from "./views/guide";
import { home, pasteForm, pasteList, pasteView } from "./views/pastes";

export function navigate(path: string): void {
  void (async () => {
    if (!(await confirmDiscardChanges())) return;
    clearUnsavedChangesGuard();
    history.pushState({}, "", path);
    await route();
  })();
}

export function handlePopState(previousPath: string): void {
  void (async () => {
    if (!(await confirmDiscardChanges())) {
      history.pushState({}, "", previousPath);
      return;
    }
    clearUnsavedChangesGuard();
    await route();
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
