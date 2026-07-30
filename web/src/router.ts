import { errorView, layout } from "./ui";
import { accountView, inviteView, loginView, passwordView } from "./views/account";
import { adminPastes, adminView } from "./views/admin";
import { guideView } from "./views/guide";
import { home, pasteForm, pasteList, pasteView } from "./views/pastes";

export function navigate(path: string): void {
  history.pushState({}, "", path);
  void route();
}

export async function route(): Promise<void> {
  const path = location.pathname;
  try {
    if (path === "/") return await home();
    if (path === "/explore") return await pasteList(false);
    if (path === "/login") return loginView();
    if (path === "/new") return pasteForm();
    if (path === "/pastes") return await pasteList(true);
    if (path === "/account") return await accountView();
    if (path === "/account/password") return passwordView();
    if (path === "/admin") return adminView();
    if (path === "/admin/pastes") return await adminPastes();
    if (path === "/guide") return guideView();
    if (path.startsWith("/invite/")) return inviteView(path.slice(8));
    const edit = path.match(/^\/pastes\/([^/]+)\/edit$/);
    if (edit?.[1]) return await pasteForm(edit[1]);
    const view = path.match(/^\/pastes\/([^/]+)$/);
    if (view?.[1]) return await pasteView(view[1]);
    layout(
      `<section class="empty"><h1>Page not found</h1><p>The requested page does not exist.</p></section>`
    );
  } catch (error) {
    errorView(error);
  }
}
