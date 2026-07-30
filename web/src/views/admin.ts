import { api } from "../api";
import { navigate } from "../router";
import { state } from "../state";
import type { Page, Paste } from "../types";
import { esc, layout } from "../ui";
import { pagination, pasteRows } from "./pastes";

export function adminView(): void {
  if (state.session.user?.role !== "admin") return navigate("/");
  layout(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Admin</h1></div></div>
    <div class="admin-links"><a href="/admin/pastes" data-link><i data-icon="file-text"></i><div><strong>All pastes</strong><span>Search, filter, edit, and remove pastes</span></div></a>
    <button type="button" data-action="admin-users"><i data-icon="user-round"></i><div><strong>Users</strong><span>Roles and account access</span></div></button>
    <button type="button" data-action="admin-invites"><i data-icon="plus"></i><div><strong>Invitations</strong><span>Create and revoke invitation links</span></div></button>
    <button type="button" data-action="admin-keys"><i data-icon="key-round"></i><div><strong>API keys</strong><span>Review and revoke keys</span></div></button></div>
    <section id="admin-detail" class="panel hidden"></section></section>`);
}

export async function adminPastes(): Promise<void> {
  if (state.session.user?.role !== "admin") return navigate("/");
  const params = new URLSearchParams(location.search); params.set("page_size", "100");
  const page = await api<Page<Paste>>(`/admin/pastes?${params}`);
  layout(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div><a class="button" href="/admin" data-link>Admin home</a></div>
    <form class="filters admin-filters" id="paste-filters"><label><span>Search</span><input name="search" value="${esc(params.get("search") ?? "")}"></label>
    <label><span>Access</span><select name="access"><option value="">All</option>${["public","unlisted","owner"].map(v=>`<option ${params.get("access")===v?"selected":""}>${v}</option>`).join("")}</select></label>
    <label><span>Owner ID</span><input type="number" name="owner_user_id" value="${esc(params.get("owner_user_id") ?? "")}"></label><button class="button" type="submit">Filter</button></form>
    <p class="result-count">${page.total} pastes</p>${pasteRows(page.items, true)}${pagination(page)}</section>`);
}
