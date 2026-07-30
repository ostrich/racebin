import { requestApi } from "../api";
import { renderIcons } from "../icons";
import type { ApiKey, User } from "../types";
import { formatDate, escapeHtml } from "../ui";

export async function loadAdmin(section: string): Promise<void> {
  const detail = document.querySelector<HTMLElement>("#admin-detail")!;
  detail.classList.remove("hidden");
  if (section === "users") {
    const users = await requestApi<Array<{id:number;username:string;role:string;enabled:boolean}>>("/admin/users");
    detail.innerHTML = `<h2>Users</h2><div class="table">${users.map(u=>`<div><strong>${escapeHtml(u.username)}</strong><select data-user-role="${u.id}"><option value="user" ${u.role==="user"?"selected":""}>User</option><option value="admin" ${u.role==="admin"?"selected":""}>Admin</option></select><label class="check"><input type="checkbox" data-user-enabled="${u.id}" ${u.enabled?"checked":""}><span>Enabled</span></label></div>`).join("")}</div>`;
  } else if (section === "invitations") {
    const invitations = await requestApi<Array<{id:number;token_prefix:string;expires_at:number;status:string;redeemed_by_username:string|null}>>("/admin/invitations");
    detail.innerHTML = `<div class="section-heading"><h2>Invitations</h2><button class="button primary" data-action="create-invitation">Create invitation</button></div><div class="table">${invitations.map(invitation=>`<div data-id="${invitation.id}"><code>${escapeHtml(invitation.token_prefix)}…</code><span>${invitation.status === "Redeemed" && invitation.redeemed_by_username ? `Redeemed by ${escapeHtml(invitation.redeemed_by_username)}` : `${escapeHtml(invitation.status)} · ${formatDate(invitation.expires_at)}`}</span>${invitation.status==="Active" ? `<button class="button" data-action="revoke-invitation">Revoke</button>` : `<span></span>`}</div>`).join("")}</div>`;
  } else {
    const [keys, users] = await Promise.all([
      requestApi<ApiKey[]>("/admin/api-keys"),
      requestApi<User[]>("/admin/users")
    ]);
    const ownerNames = new Map(users.map(user => [user.id, user.username]));
    detail.innerHTML = `<h2>API keys</h2><div class="table">${keys.map(key=>`<div class="admin-key-row" data-id="${key.id}">
      <div class="admin-key-identity"><strong>${escapeHtml(key.name)}</strong><code>${escapeHtml(key.token_prefix)}</code></div>
      <div class="admin-key-access"><span>Owner: ${key.user_id === null ? "No owner" : escapeHtml(ownerNames.get(key.user_id) ?? `User #${key.user_id}`)}</span>
      <div class="admin-key-scopes" aria-label="Privileges">${key.scopes.length ? key.scopes.map(scope => `<code>${escapeHtml(scope)}</code>`).join("") : `<span>No privileges</span>`}</div></div>
      <div class="admin-key-actions"><label class="check"><input type="checkbox" data-admin-key="${key.id}" ${key.enabled?"checked":""}><span>Enabled</span></label><button class="icon-button" title="Delete API key" aria-label="Delete API key" data-action="delete-admin-key"><i data-icon="trash-2"></i></button></div>
    </div>`).join("")}</div>`;
  }
  renderIcons();
}
