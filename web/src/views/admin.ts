import { requestApi } from "../api";
import { navigate } from "../router";
import { state } from "../state";
import type { Page, Paste, User } from "../types";
import { escapeHtml, renderLayout } from "../ui";
import { pagination, pasteRows } from "./pastes";

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
  const params = new URLSearchParams(location.search); params.set("page_size", "100");
  const [page, users] = await Promise.all([
    requestApi<Page<Paste>>(`/admin/pastes?${params}`),
    requestApi<User[]>("/admin/users")
  ]);
  const ownerNames = new Map(users.map(user => [user.id, user.username]));
  renderLayout(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div><a class="button" href="/admin" data-link>Admin home</a></div>
    <form class="filters admin-filters" id="paste-filters"><label><span>Search</span><input name="search" value="${escapeHtml(params.get("search") ?? "")}"></label>
    <label><span>Visibility</span><select name="visibility"><option value="">All</option>${["public","unlisted","private"].map(value=>`<option ${params.get("visibility")===value?"selected":""}>${value}</option>`).join("")}</select></label>
    <label><span>Owner ID</span><input type="number" name="owner_id" value="${escapeHtml(params.get("owner_id") ?? "")}"></label><button class="button" type="submit">Filter</button></form>
    <p class="result-count">${page.total_items} pastes</p>${pasteRows(page.items, { manage: true, ownerNames })}${pagination(page)}</section>`);
}
