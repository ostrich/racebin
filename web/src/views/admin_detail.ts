import { requestApi } from "../api";
import { renderIcons } from "../icons";
import type { ApiKey } from "../types";
import { formatDate, escapeHtml } from "../ui";

export async function loadAdmin(section: string): Promise<void> {
  const detail = document.querySelector<HTMLElement>("#admin-detail")!;
  detail.classList.remove("hidden");
  if (section === "users") {
    const users = await requestApi<Array<{id:number;username:string;role:string;enabled:boolean}>>("/admin/users");
    detail.innerHTML = `<h2>Users</h2><div class="table">${users.map(u=>`<div><strong>${escapeHtml(u.username)}</strong><select data-user-role="${u.id}"><option value="user" ${u.role==="user"?"selected":""}>User</option><option value="admin" ${u.role==="admin"?"selected":""}>Admin</option></select><label class="check"><input type="checkbox" data-user-enabled="${u.id}" ${u.enabled?"checked":""}><span>Enabled</span></label></div>`).join("")}</div>`;
  } else if (section === "invitations") {
    const invitations = await requestApi<Array<{id:number;token_prefix:string;expires_at:number;status:string}>>("/admin/invitations");
    detail.innerHTML = `<div class="section-heading"><h2>Invitations</h2><button class="button primary" data-action="create-invitation">Create invitation</button></div><div class="table">${invitations.map(invitation=>`<div data-id="${invitation.id}"><code>${escapeHtml(invitation.token_prefix)}…</code><span>${escapeHtml(invitation.status)} · ${formatDate(invitation.expires_at)}</span>${invitation.status==="Active" ? `<button class="button" data-action="revoke-invitation">Revoke</button>` : `<span></span>`}</div>`).join("")}</div>`;
  } else {
    const keys = await requestApi<ApiKey[]>("/admin/api-keys");
    detail.innerHTML = `<h2>API keys</h2><div class="table">${keys.map(k=>`<div data-id="${k.id}"><div><strong>${escapeHtml(k.name)}</strong><br><code>${escapeHtml(k.token_prefix)}</code></div><label class="check"><input type="checkbox" data-admin-key="${k.id}" ${k.enabled?"checked":""}><span>Enabled</span></label><button class="icon-button" title="Delete API key" aria-label="Delete API key" data-action="delete-admin-key"><i data-icon="trash-2"></i></button></div>`).join("")}</div>`;
  }
  renderIcons();
}
