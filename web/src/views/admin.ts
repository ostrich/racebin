import { requestApi } from "../api";
import { navigate } from "../router";
import { state } from "../state";
import type { Page, Paste, User } from "../types";
import { escapeHtml, formatDate, iconButton, pasteDisplayTitle, renderLayout } from "../ui";
import { formatByteSize, pagination, pasteFilters, pasteFormatLabel } from "./pastes";

export function adminView(): void {
  if (state.session.user?.role !== "admin") return navigate("/");
  renderLayout(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Admin</h1></div></div>
    <div class="admin-links"><a href="/admin/pastes" data-link><i data-icon="file-text"></i><div><strong>All pastes</strong><span>Search, filter, edit, and remove pastes</span></div></a>
    <button type="button" data-action="admin-users"><i data-icon="user-round"></i><div><strong>Users</strong><span>Roles and account access</span></div></button>
    <button type="button" data-action="admin-invitations"><i data-icon="plus"></i><div><strong>Invitations</strong><span>Create and revoke invitation links</span></div></button>
    <button type="button" data-action="admin-keys"><i data-icon="key-round"></i><div><strong>API keys</strong><span>Review and revoke keys</span></div></button></div>
    <section id="admin-detail" class="panel hidden"></section></section>`);
}

export async function adminPastes(): Promise<void> {
  if (state.session.user?.role !== "admin") return navigate("/");
  const params = new URLSearchParams(location.search);
  const requestParams = new URLSearchParams(params);
  requestParams.set("page_size", "100");
  const [page, users] = await Promise.all([
    requestApi<Page<Paste>>(`/admin/pastes?${requestParams}`),
    requestApi<User[]>("/admin/users")
  ]);
  const ownerNames = new Map(users.map(user => [user.id, user.username]));
  const rows = page.items.length ? `<div class="admin-paste-list">${page.items.map(paste => {
    const owner = paste.owner_id === null
      ? `<span class="muted">No owner</span>`
      : `<a href="/admin/pastes?${ownerFilter(params, paste.owner_id)}" data-link><strong>${escapeHtml(ownerNames.get(paste.owner_id) ?? `User #${paste.owner_id}`)}</strong><small>User #${paste.owner_id}</small></a>`;
    return `<article class="admin-paste-row paste-row">
      <div class="paste-main"><a class="paste-title" href="/pastes/${escapeHtml(paste.id)}" data-link>${escapeHtml(pasteDisplayTitle(paste))}</a><p>${escapeHtml(paste.content.slice(0, 160).replace(/\s+/g, " "))}</p><code>${escapeHtml(paste.id)}</code></div>
      <div class="admin-paste-owner">${owner}</div>
      <div class="paste-meta">${adminFilterBadge(params, pasteFormatLabel(paste), paste.content_kind === "text" ? "language" : "content_kind", paste.content_kind === "text" ? paste.language : paste.content_kind)}${adminFilterBadge(params, paste.visibility, "visibility", paste.visibility)}${paste.attachment_count ? adminFilterBadge(params, `${paste.attachment_count} attachment${paste.attachment_count === 1 ? "" : "s"}`, "has_attachments", "true") : ""}<span>${formatByteSize(paste.size_bytes)}</span></div>
      <time>${formatDate(paste.created_at)}</time>
      <div class="row-actions">${iconButton("copy", "Copy link")}<a class="icon-button" title="Edit" aria-label="Edit" href="/pastes/${escapeHtml(paste.id)}/edit" data-link><i data-icon="edit-3"></i></a>${iconButton("trash-2", "Delete")}</div>
      <input type="hidden" value="${escapeHtml(paste.id)}">
    </article>`;
  }).join("")}</div>` : `<div class="empty compact"><p>No pastes found.</p></div>`;
  renderLayout(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div><a class="button" href="/admin" data-link>Admin home</a></div>
    ${pasteFilters(params, "admin", ownerNames)}
    <p class="result-count">${page.total_items} pastes</p>
    <div class="admin-paste-head" aria-hidden="true"><span>Paste</span><span>Owner</span><span>Metadata</span><span>Created</span><span>Actions</span></div>
    ${rows}${pagination(page)}</section>`);
}

function ownerFilter(params: URLSearchParams, ownerId: number): string {
  const next = new URLSearchParams(params);
  next.set("owner_id", String(ownerId));
  next.delete("page");
  return escapeHtml(next.toString());
}

function adminFilterBadge(
  params: URLSearchParams,
  label: string,
  key: string,
  value: string
): string {
  const next = new URLSearchParams(params);
  next.set(key, value);
  next.delete("page");
  return `<a href="/admin/pastes?${escapeHtml(next.toString())}" data-link>${escapeHtml(label)}</a>`;
}
