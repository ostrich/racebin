import { api } from "../api";
import { renderIcons } from "../icons";
import type { ApiKey } from "../types";
import { date, esc } from "../ui";

export async function loadAdmin(section: string): Promise<void> {
  const detail = document.querySelector<HTMLElement>("#admin-detail")!;
  detail.classList.remove("hidden");
  if (section === "users") {
    const users = await api<Array<{id:number;username:string;role:string;enabled:boolean}>>("/admin/users");
    detail.innerHTML = `<h2>Users</h2><div class="table">${users.map(u=>`<div><strong>${esc(u.username)}</strong><select data-user-role="${u.id}"><option value="user" ${u.role==="user"?"selected":""}>User</option><option value="admin" ${u.role==="admin"?"selected":""}>Admin</option></select><label class="check"><input type="checkbox" data-user-enabled="${u.id}" ${u.enabled?"checked":""}><span>Enabled</span></label></div>`).join("")}</div>`;
  } else if (section === "invites") {
    const invites = await api<Array<{id:number;token_prefix:string;expires:number;status:string}>>("/admin/invites");
    detail.innerHTML = `<div class="section-heading"><h2>Invitations</h2><button class="button primary" data-action="create-invite">Create invitation</button></div><div class="table">${invites.map(i=>`<div data-id="${i.id}"><code>${esc(i.token_prefix)}…</code><span>${esc(i.status)} · ${date(i.expires)}</span>${i.status==="Active" ? `<button class="button" data-action="revoke-invite">Revoke</button>` : `<span></span>`}</div>`).join("")}</div>`;
  } else {
    const keys = await api<ApiKey[]>("/admin/api-keys");
    detail.innerHTML = `<h2>API keys</h2><div class="table">${keys.map(k=>`<div data-id="${k.id}"><div><strong>${esc(k.name)}</strong><br><code>${esc(k.prefix)}</code></div><label class="check"><input type="checkbox" data-admin-key="${k.id}" ${k.enabled?"checked":""}><span>Enabled</span></label><button class="icon-button" title="Delete API key" aria-label="Delete API key" data-action="delete-admin-key"><i data-icon="trash-2"></i></button></div>`).join("")}</div>`;
  }
  renderIcons();
}
