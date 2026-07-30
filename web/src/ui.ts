import { renderIcons } from "./icons";
import { state } from "./state";
import type { Paste } from "./types";

const app = document.querySelector<HTMLDivElement>("#app")!;

export const escapeHtml = (value: unknown) =>
  String(value ?? "").replace(
    /[&<>"']/g,
    character =>
      ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;"
      })[character]!
  );

export const formatDate = (value: number | null) =>
  value
    ? new Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "short"
      }).format(value * 1000)
    : "Never";

export const pasteDisplayTitle = (paste: Paste) => paste.title || paste.id;

export const iconButton = (name: string, label: string) =>
  `<button class="icon-button" type="button" title="${escapeHtml(label)}" aria-label="${escapeHtml(label)}" data-action="${name}"><i data-icon="${name}"></i></button>`;

export const formValue = (form: FormData, key: string) =>
  String(form.get(key) ?? "");

export function renderLayout(content: string): void {
  const user = state.session.user;
  app.innerHTML = `
    <header>
      <a class="brand" href="/" data-link>${escapeHtml(state.config.site_name)}</a>
      <nav>
        <a href="/explore" data-link>Explore</a>
        ${user ? `<a href="/pastes" data-link>My pastes</a><a href="/pastes/new" data-link><i data-icon="plus"></i> New</a>` : ""}
        ${user?.role === "admin" ? `<a href="/admin" data-link>Admin</a>` : ""}
      </nav>
      <div class="session">
        ${
          user
            ? `<a href="/account" data-link><i data-icon="user-round"></i><span>${escapeHtml(user.username)}</span></a>${iconButton("log-out", "Log out")}`
            : `<a href="/login" data-link><i data-icon="log-in"></i><span>Log in</span></a>`
        }
      </div>
    </header>
    <main>${content}</main>
    <div id="toast" role="status" aria-live="polite"></div>`;
  renderIcons();
}

export function showNotice(message: string, variant = ""): void {
  const toast = document.querySelector<HTMLDivElement>("#toast");
  if (!toast) return;
  toast.textContent = message;
  toast.className = `show ${variant}`;
  window.setTimeout(() => (toast.className = ""), 3500);
}

export function errorView(error: unknown): void {
  const message = error instanceof Error ? error.message : "The request failed.";
  renderLayout(
    `<section class="empty"><h1>Unable to load this page</h1><p>${escapeHtml(message)}</p><a class="button" href="/" data-link>Return home</a></section>`
  );
}
